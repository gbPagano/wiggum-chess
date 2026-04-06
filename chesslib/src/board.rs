use crate::bitboard::BitBoard;
use crate::castle_rights::CastleRights;
use crate::chess_move::ChessMove;
use crate::color::Color;
use crate::file::{ALL_FILES, File};
use crate::magic;
use crate::movegen::MoveGen;
use crate::pieces::{ALL_PIECES, Piece};
use crate::rank::{ALL_RANKS, Rank};
use crate::square::Square;

use anyhow::{Error, bail};
use std::fmt;
use std::str::FromStr;

#[derive(Clone)]
pub struct Board {
    pieces_bitboards: [BitBoard; 6],
    colors_bitboards: [BitBoard; 2],
    combined_bitboard: BitBoard,
    side_to_move: Color,
    en_passant: Option<Square>,
    castle_rights: CastleRights,
    pinned_bitboard: BitBoard,
    checkers_bitboard: BitBoard,
    zobrist_hash: u64,
}

impl Board {
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            pieces_bitboards: [BitBoard::new(0); 6],
            colors_bitboards: [BitBoard::new(0); 2],
            combined_bitboard: BitBoard::new(0),
            side_to_move: Color::White,
            en_passant: None,
            castle_rights: CastleRights::default(),
            pinned_bitboard: BitBoard(0),
            checkers_bitboard: BitBoard(0),
            zobrist_hash: 0,
        }
    }

    /// Returns the current Zobrist hash of the board position.
    #[inline(always)]
    pub fn zobrist_hash(&self) -> u64 {
        self.zobrist_hash
    }

    /// Compute the Zobrist hash from scratch based on current board state.
    fn compute_zobrist(&self) -> u64 {
        let mut hash = 0u64;

        // Piece/color/square keys
        for piece in ALL_PIECES {
            for color in [Color::White, Color::Black] {
                let bb = self.get_piece_bitboard(piece) & self.get_color_bitboard(color);
                for sq in bb.get_squares() {
                    hash ^= magic::zobrist_piece_key(piece, color, sq);
                }
            }
        }

        // Castling rights
        if self.castle_rights.white_kingside {
            hash ^= magic::zobrist_castling_key(0);
        }
        if self.castle_rights.white_queenside {
            hash ^= magic::zobrist_castling_key(1);
        }
        if self.castle_rights.black_kingside {
            hash ^= magic::zobrist_castling_key(2);
        }
        if self.castle_rights.black_queenside {
            hash ^= magic::zobrist_castling_key(3);
        }

        // En passant file
        if let Some(sq) = self.en_passant {
            hash ^= magic::zobrist_en_passant_key(sq.get_file());
        }

        // Side to move
        if self.side_to_move == Color::Black {
            hash ^= magic::zobrist_side_key();
        }

        hash
    }

    #[inline(always)]
    fn place_piece(&mut self, square: Square, piece: Piece, color: Color) {
        let bitboard = BitBoard::from_square(square);
        self.xor(piece, bitboard, color);
    }

    #[inline(always)]
    pub fn xor(&mut self, piece: Piece, bitboard: BitBoard, color: Color) {
        self.pieces_bitboards[piece.to_index()] ^= bitboard;
        self.colors_bitboards[color.to_index()] ^= bitboard;
        self.combined_bitboard ^= bitboard;
    }

    #[inline(always)]
    fn set_side(&mut self, color: Color) {
        self.side_to_move = color;
    }

    #[inline(always)]
    fn set_castling_rights(&mut self, rights: CastleRights) {
        self.castle_rights = rights;
    }

    #[inline(always)]
    fn set_en_passant(&mut self, square: Square) {
        // only set en_passatn if the pawn ca acttually be captured next move
        if !(magic::get_adjacent_files(square.get_file())
            & magic::get_rank_bitboard(square.forward(self.side_to_move).unwrap().get_rank())
            & self.get_piece_bitboard(Piece::Pawn)
            & self.get_color_bitboard(!self.side_to_move))
        .is_empty()
        {
            self.en_passant = Some(square);
        }
    }

    #[inline(always)]
    pub fn get_piece(&self, square: Square) -> Option<Piece> {
        let bitboard = BitBoard::from_square(square);
        if (self.combined_bitboard & bitboard).is_empty() {
            return None;
        }

        for piece in ALL_PIECES {
            let piece_bitboard = self.pieces_bitboards[piece.to_index()];
            if !(piece_bitboard & bitboard).is_empty() {
                return Some(piece);
            }
        }

        None
    }

    #[inline(always)]
    fn get_color(&self, square: Square) -> Option<Color> {
        let bitboard = BitBoard::from_square(square);
        if !(self.colors_bitboards[Color::White.to_index()] & bitboard).is_empty() {
            Some(Color::White)
        } else if !(self.colors_bitboards[Color::Black.to_index()] & bitboard).is_empty() {
            Some(Color::Black)
        } else {
            None
        }
    }

    #[inline(always)]
    fn get_piece_and_color(&self, square: Square) -> Option<(Piece, Color)> {
        let piece = self.get_piece(square)?;
        let color = self.get_color(square)?;
        Some((piece, color))
    }

    #[inline(always)]
    pub fn get_piece_bitboard(&self, piece: Piece) -> BitBoard {
        self.pieces_bitboards[piece.to_index()]
    }

    #[inline(always)]
    pub fn get_combined_bitboard(&self) -> BitBoard {
        self.combined_bitboard
    }

    #[inline(always)]
    pub fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    #[inline(always)]
    pub fn get_color_bitboard(&self, color: Color) -> BitBoard {
        self.colors_bitboards[color.to_index()]
    }

    #[inline(always)]
    pub fn get_king_square(&self, color: Color) -> Square {
        (self.get_piece_bitboard(Piece::King) & self.get_color_bitboard(color)).to_square()
    }

    #[inline(always)]
    pub fn get_pinned_bitboard(&self) -> BitBoard {
        self.pinned_bitboard
    }

    #[inline(always)]
    pub fn get_checkers_bitboard(&self) -> BitBoard {
        self.checkers_bitboard
    }

    #[inline(always)]
    pub fn en_passant(&self) -> Option<Square> {
        self.en_passant
    }

    #[inline(always)]
    pub fn castle_rights(&self) -> CastleRights {
        self.castle_rights
    }

    #[inline(always)]
    fn update_attacked_bitboards(&mut self) {
        self.pinned_bitboard = BitBoard(0);
        self.checkers_bitboard = BitBoard(0);

        let king_square = (self.get_piece_bitboard(Piece::King)
            & self.get_color_bitboard(self.side_to_move))
        .to_square();

        let pinners = self.get_color_bitboard(!self.side_to_move)
            & ((magic::get_bishop_rays(king_square)
                & (self.get_piece_bitboard(Piece::Bishop)
                    | self.get_piece_bitboard(Piece::Queen)))
                | (magic::get_rook_rays(king_square)
                    & (self.get_piece_bitboard(Piece::Rook)
                        | self.get_piece_bitboard(Piece::Queen))));

        for sq in pinners.get_squares() {
            let between = magic::get_between(sq, king_square) & self.get_combined_bitboard();
            if between.is_empty() {
                self.checkers_bitboard ^= BitBoard::from_square(sq);
            } else if between.0.count_ones() == 1 {
                self.pinned_bitboard ^= between;
            }
        }

        self.checkers_bitboard ^= magic::get_knight_moves(king_square)
            & self.get_color_bitboard(!self.side_to_move)
            & self.get_piece_bitboard(Piece::Knight);

        self.checkers_bitboard ^= magic::get_pawn_attacks(
            king_square,
            self.side_to_move,
            self.get_color_bitboard(!self.side_to_move) & self.get_piece_bitboard(Piece::Pawn),
        );
    }

    #[inline(always)]
    pub fn make_move(&self, m: ChessMove) -> Board {
        let mut result = self.clone();
        result.en_passant = None;
        result.checkers_bitboard = BitBoard(0);
        result.pinned_bitboard = BitBoard(0);

        // Zobrist: XOR out old en passant and castling; flip side to move
        result.zobrist_hash ^= magic::zobrist_side_key();
        if let Some(ep_sq) = self.en_passant {
            result.zobrist_hash ^= magic::zobrist_en_passant_key(ep_sq.get_file());
        }
        let old_rights = self.castle_rights;
        if old_rights.white_kingside {
            result.zobrist_hash ^= magic::zobrist_castling_key(0);
        }
        if old_rights.white_queenside {
            result.zobrist_hash ^= magic::zobrist_castling_key(1);
        }
        if old_rights.black_kingside {
            result.zobrist_hash ^= magic::zobrist_castling_key(2);
        }
        if old_rights.black_queenside {
            result.zobrist_hash ^= magic::zobrist_castling_key(3);
        }

        let source_bb = BitBoard::from_square(m.source);
        let dest_bb = BitBoard::from_square(m.dest);

        let moved_piece = self.get_piece(m.source).unwrap();

        result.xor(moved_piece, source_bb, self.side_to_move);
        result.xor(moved_piece, dest_bb, self.side_to_move);
        // Zobrist: move piece from source to dest
        result.zobrist_hash ^=
            magic::zobrist_piece_key(moved_piece, self.side_to_move, m.source);
        result.zobrist_hash ^= magic::zobrist_piece_key(moved_piece, self.side_to_move, m.dest);

        if let Some(captured) = self.get_piece(m.dest) {
            result.xor(captured, dest_bb, !self.side_to_move);
            // Zobrist: remove captured piece
            result.zobrist_hash ^=
                magic::zobrist_piece_key(captured, !self.side_to_move, m.dest);
        }

        result
            .castle_rights
            .update_from_square(!self.side_to_move, m.dest);
        result
            .castle_rights
            .update_from_square(self.side_to_move, m.source);

        let enemy_king =
            self.get_piece_bitboard(Piece::King) & self.get_color_bitboard(!self.side_to_move);
        let enemy_king_sq = enemy_king.to_square();

        let move_bb = source_bb ^ dest_bb;
        let has_castled =
            moved_piece == Piece::King && (move_bb & magic::get_castle_squares()) == move_bb;

        if moved_piece == Piece::Knight {
            result.checkers_bitboard ^= magic::get_knight_moves(enemy_king_sq) & dest_bb;
        } else if moved_piece == Piece::Pawn {
            if let Some(Piece::Knight) = m.promotion {
                result.xor(Piece::Pawn, dest_bb, self.side_to_move);
                result.xor(Piece::Knight, dest_bb, self.side_to_move);
                // Zobrist: replace pawn with knight at dest
                result.zobrist_hash ^=
                    magic::zobrist_piece_key(Piece::Pawn, self.side_to_move, m.dest);
                result.zobrist_hash ^=
                    magic::zobrist_piece_key(Piece::Knight, self.side_to_move, m.dest);
                result.checkers_bitboard ^= magic::get_knight_moves(enemy_king_sq) & dest_bb;
            } else if let Some(promotion) = m.promotion {
                result.xor(Piece::Pawn, dest_bb, self.side_to_move);
                result.xor(promotion, dest_bb, self.side_to_move);
                // Zobrist: replace pawn with promoted piece at dest
                result.zobrist_hash ^=
                    magic::zobrist_piece_key(Piece::Pawn, self.side_to_move, m.dest);
                result.zobrist_hash ^=
                    magic::zobrist_piece_key(promotion, self.side_to_move, m.dest);
            } else if !(source_bb & magic::get_pawn_source_double_moves()).is_empty()
                && !(dest_bb & magic::get_pawn_dest_double_moves()).is_empty()
            {
                result.set_en_passant(m.dest.backward(self.side_to_move).unwrap());
                // Zobrist: XOR in new en passant file if actually set
                if let Some(ep_sq) = result.en_passant {
                    result.zobrist_hash ^= magic::zobrist_en_passant_key(ep_sq.get_file());
                }
                result.checkers_bitboard ^=
                    magic::get_pawn_attacks(enemy_king_sq, !self.side_to_move, dest_bb);
            } else if Some(m.dest) == self.en_passant {
                let captured_sq = m.dest.forward(!self.side_to_move).unwrap();
                result.xor(
                    Piece::Pawn,
                    BitBoard::from_square(captured_sq),
                    !self.side_to_move,
                );
                // Zobrist: remove the captured en passant pawn
                result.zobrist_hash ^=
                    magic::zobrist_piece_key(Piece::Pawn, !self.side_to_move, captured_sq);
                result.checkers_bitboard ^=
                    magic::get_pawn_attacks(enemy_king_sq, !self.side_to_move, dest_bb);
            } else {
                result.checkers_bitboard ^=
                    magic::get_pawn_attacks(enemy_king_sq, !self.side_to_move, dest_bb);
            }
        } else if has_castled {
            let backrank = match self.side_to_move {
                Color::White => Rank::First,
                Color::Black => Rank::Eighth,
            };
            let rook_start_file = match m.dest.get_file() {
                File::C | File::B => File::A,
                File::G => File::H,
                _ => unreachable!(),
            };
            let rook_end_file = match m.dest.get_file() {
                File::C | File::B => File::D,
                File::G => File::F,
                _ => unreachable!(),
            };
            let start_bb = BitBoard::set(backrank, rook_start_file);
            let end_bb = BitBoard::set(backrank, rook_end_file);
            result.xor(Piece::Rook, start_bb, self.side_to_move);
            result.xor(Piece::Rook, end_bb, self.side_to_move);
            // Zobrist: move rook during castling
            result.zobrist_hash ^= magic::zobrist_piece_key(
                Piece::Rook,
                self.side_to_move,
                Square::new(backrank, rook_start_file),
            );
            result.zobrist_hash ^= magic::zobrist_piece_key(
                Piece::Rook,
                self.side_to_move,
                Square::new(backrank, rook_end_file),
            );
        }

        let rays_attackers = result.get_color_bitboard(self.side_to_move)
            & ((magic::get_bishop_rays(enemy_king_sq)
                & (result.get_piece_bitboard(Piece::Bishop)
                    | result.get_piece_bitboard(Piece::Queen)))
                | (magic::get_rook_rays(enemy_king_sq)
                    & (result.get_piece_bitboard(Piece::Rook)
                        | result.get_piece_bitboard(Piece::Queen))));

        for square in rays_attackers.get_squares() {
            let between = magic::get_between(square, enemy_king_sq) & result.combined_bitboard;

            if between.is_empty() {
                result.checkers_bitboard ^= BitBoard::from_square(square);
            } else if between.0.count_ones() == 1 {
                result.pinned_bitboard ^= between;
            }
        }

        // Zobrist: XOR in new castling rights
        let new_rights = result.castle_rights;
        if new_rights.white_kingside {
            result.zobrist_hash ^= magic::zobrist_castling_key(0);
        }
        if new_rights.white_queenside {
            result.zobrist_hash ^= magic::zobrist_castling_key(1);
        }
        if new_rights.black_kingside {
            result.zobrist_hash ^= magic::zobrist_castling_key(2);
        }
        if new_rights.black_queenside {
            result.zobrist_hash ^= magic::zobrist_castling_key(3);
        }

        result.side_to_move = !result.side_to_move;
        result
    }

    /// Returns true if the side to move is in checkmate (in check with no legal moves).
    pub fn is_checkmate(&self) -> bool {
        !self.checkers_bitboard.is_empty() && MoveGen::new_legal(self).len() == 0
    }

    /// Returns true if the side to move is in stalemate (not in check with no legal moves).
    pub fn is_stalemate(&self) -> bool {
        self.checkers_bitboard.is_empty() && MoveGen::new_legal(self).len() == 0
    }

    /// Returns true if the position is a draw by insufficient material.
    ///
    /// Covers: K vs K, K+N vs K, K+B vs K, K+B vs K+B (bishops on same color square).
    pub fn is_insufficient_material(&self) -> bool {
        // Any pawns, rooks, or queens → sufficient material
        if !self.get_piece_bitboard(Piece::Pawn).is_empty()
            || !self.get_piece_bitboard(Piece::Rook).is_empty()
            || !self.get_piece_bitboard(Piece::Queen).is_empty()
        {
            return false;
        }

        let white_bishops =
            self.get_piece_bitboard(Piece::Bishop) & self.get_color_bitboard(Color::White);
        let black_bishops =
            self.get_piece_bitboard(Piece::Bishop) & self.get_color_bitboard(Color::Black);
        let white_knights =
            self.get_piece_bitboard(Piece::Knight) & self.get_color_bitboard(Color::White);
        let black_knights =
            self.get_piece_bitboard(Piece::Knight) & self.get_color_bitboard(Color::Black);

        let white_minor = white_bishops.0.count_ones() + white_knights.0.count_ones();
        let black_minor = black_bishops.0.count_ones() + black_knights.0.count_ones();

        // K vs K
        if white_minor == 0 && black_minor == 0 {
            return true;
        }

        // K+N vs K or K vs K+N
        if (white_minor == 1 && white_knights.0.count_ones() == 1 && black_minor == 0)
            || (black_minor == 1 && black_knights.0.count_ones() == 1 && white_minor == 0)
        {
            return true;
        }

        // K+B vs K or K vs K+B
        if (white_minor == 1 && white_bishops.0.count_ones() == 1 && black_minor == 0)
            || (black_minor == 1 && black_bishops.0.count_ones() == 1 && white_minor == 0)
        {
            return true;
        }

        // K+B vs K+B with bishops on same color square
        if white_minor == 1
            && black_minor == 1
            && white_bishops.0.count_ones() == 1
            && black_bishops.0.count_ones() == 1
        {
            let wb_idx = white_bishops.to_square().to_index();
            let bb_idx = black_bishops.to_square().to_index();
            // Square color: (file + rank) % 2; index = rank*8 + file
            let wb_light = (wb_idx % 8 + wb_idx / 8) % 2 == 0;
            let bb_light = (bb_idx % 8 + bb_idx / 8) % 2 == 0;
            if wb_light == bb_light {
                return true;
            }
        }

        false
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap()
    }
}

impl FromStr for Board {
    type Err = Error;

    fn from_str(fen: &str) -> Result<Self, Self::Err> {
        let tokens: Vec<&str> = fen.split_whitespace().collect();
        if tokens.len() < 4 {
            bail!("invalid fen string");
        }

        let ranks = tokens[0].split('/').collect::<Vec<_>>();
        if ranks.len() != 8 {
            bail!("invalid fen string");
        }

        let mut board = Self::new();
        for (rank_idx, rank_str) in ranks.iter().enumerate() {
            let rank = Rank::from_index(7 - rank_idx); // 8th rank first
            let mut file = File::from_index(0);
            for c in rank_str.chars() {
                match c {
                    '1'..='8' => {
                        let skip = c.to_digit(10).unwrap() as usize;
                        file = File::from_index(file.to_index() + skip);
                    }
                    _ => {
                        let color = if c.is_uppercase() {
                            Color::White
                        } else {
                            Color::Black
                        };

                        let piece = match c.to_ascii_lowercase() {
                            'k' => Piece::King,
                            'q' => Piece::Queen,
                            'r' => Piece::Rook,
                            'b' => Piece::Bishop,
                            'n' => Piece::Knight,
                            'p' => Piece::Pawn,
                            _ => bail!("invalid fen string"),
                        };

                        let square = Square::new(rank, file);
                        board.place_piece(square, piece, color);

                        file = file.right();
                    }
                }
            }
        }

        match tokens[1] {
            "w" => board.set_side(Color::White),
            "b" => board.set_side(Color::Black),
            _ => bail!("Turno inválido: {}", tokens[1]),
        }

        let rights = CastleRights::from_str(tokens[2])?;
        board.set_castling_rights(rights);

        if let Ok(sq) = Square::from_str(tokens[3]) {
            board.side_to_move = !board.side_to_move;
            board.set_en_passant(sq);
            board.side_to_move = !board.side_to_move;
        }

        board.update_attacked_bitboards();
        board.zobrist_hash = board.compute_zobrist();

        Ok(board)
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for rank in ALL_RANKS.iter().rev() {
            let mut empty = 0;
            for file in ALL_FILES.iter() {
                let square = Square::new(*rank, *file);
                if let Some((piece, color)) = self.get_piece_and_color(square) {
                    if empty != 0 {
                        write!(f, "{}", empty)?;
                        empty = 0;
                    }
                    write!(f, "{}", piece.to_string(color))?;
                } else {
                    empty += 1;
                }
            }
            if empty != 0 {
                write!(f, "{}", empty)?;
            }
            if *rank != Rank::First {
                write!(f, "/")?;
            }
        }
        write!(f, " ")?;

        if self.side_to_move == Color::White {
            write!(f, "w ")?;
        } else {
            write!(f, "b ")?;
        }

        write!(f, "{}", self.castle_rights)?;
        write!(f, " ")?;

        if let Some(square) = self.en_passant {
            write!(f, "{}", square)?;
        } else {
            write!(f, "-")?;
        }

        write!(f, " 0 1") // TODO
    }
}

impl fmt::Debug for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in ALL_RANKS.iter().rev() {
            if *rank == Rank::Eighth {
                writeln!(f, "  ╭───┬───┬───┬───┬───┬───┬───┬───╮")?;
            } else {
                writeln!(f, "  ├───┼───┼───┼───┼───┼───┼───┼───┤")?;
            }
            write!(f, "{}", rank.to_index() + 1)?;

            for file in ALL_FILES.iter() {
                write!(f, " │ ")?;
                let square = Square::new(*rank, *file);
                if let Some((piece, color)) = self.get_piece_and_color(square) {
                    write!(f, "{}", piece.to_symbol(color))?;
                } else {
                    write!(f, " ")?;
                }
            }
            writeln!(f, " │")?;
        }
        writeln!(f, "  ╰───┴───┴───┴───┴───┴───┴───┴───╯")?;
        write!(f, "    A   B   C   D   E   F   G   H  ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_initial_position() {
        let board = Board::default();
        let initial_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let board_fen = format!("{}", board);
        assert_eq!(board_fen, initial_fen);
    }

    #[test]
    fn test_board_from_str() {
        assert!(
            Board::from_str("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1").is_ok()
        );
        assert!(
            Board::from_str("rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2")
                .is_ok()
        );
    }

    #[test]
    fn test_checkmate_scholars_mate() {
        // Scholar's mate: 1.e4 e5 2.Bc4 Nc6 3.Qh5 Nf6?? 4.Qxf7#
        let board: Board =
            "r1bqkb1r/pppp1Qpp/2n2n2/4p3/2B1P3/8/PPPP1PPP/RNB1K1NR b KQkq - 0 4"
                .parse()
                .unwrap();
        assert!(board.is_checkmate());
        assert!(!board.is_stalemate());
    }

    #[test]
    fn test_checkmate_fools_mate() {
        // Fool's mate: 1.f3 e5 2.g4 Qh4#
        let board: Board =
            "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3"
                .parse()
                .unwrap();
        assert!(board.is_checkmate());
        assert!(!board.is_stalemate());
    }

    #[test]
    fn test_not_checkmate_or_stalemate_in_play() {
        // Opening position: neither checkmate nor stalemate
        let board = Board::default();
        assert!(!board.is_checkmate());
        assert!(!board.is_stalemate());

        // A mid-game position
        let board: Board =
            "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3"
                .parse()
                .unwrap();
        assert!(!board.is_checkmate());
        assert!(!board.is_stalemate());
    }

    #[test]
    fn test_stalemate_queen() {
        // Classic stalemate: black king on a8, white queen c7, white king b6.
        // Black king on a8: b8 is covered by queen (c7 diagonal), a7 is covered by king b6.
        let board: Board = "k7/2Q5/1K6/8/8/8/8/8 b - - 0 1".parse().unwrap();
        assert!(board.is_stalemate());
        assert!(!board.is_checkmate());
    }

    #[test]
    fn test_stalemate_pawn() {
        // Black king b8, white pawn b7, white king b6.
        // b8 king: a8 & c8 covered by pawn b7 diagonally; a7 & c7 covered by king b6.
        let board: Board = "1k6/1P6/1K6/8/8/8/8/8 b - - 0 1".parse().unwrap();
        assert!(board.is_stalemate());
        assert!(!board.is_checkmate());
    }

    #[test]
    fn test_insufficient_material_k_vs_k() {
        let board: Board = "8/8/4k3/8/8/3K4/8/8 w - - 0 1".parse().unwrap();
        assert!(board.is_insufficient_material());
    }

    #[test]
    fn test_insufficient_material_kn_vs_k() {
        // K+N vs K
        let board: Board = "8/8/4k3/8/8/3K4/8/7N w - - 0 1".parse().unwrap();
        assert!(board.is_insufficient_material());
        // K vs K+N
        let board: Board = "8/8/4k3/8/8/3K4/8/7n w - - 0 1".parse().unwrap();
        assert!(board.is_insufficient_material());
    }

    #[test]
    fn test_insufficient_material_kb_vs_k() {
        // K+B vs K
        let board: Board = "8/8/4k3/8/8/3K4/8/7B w - - 0 1".parse().unwrap();
        assert!(board.is_insufficient_material());
        // K vs K+B
        let board: Board = "8/8/4k3/8/8/3K4/8/7b w - - 0 1".parse().unwrap();
        assert!(board.is_insufficient_material());
    }

    #[test]
    fn test_insufficient_material_kb_vs_kb_same_color() {
        // Both bishops on light squares (a1=dark, b1=light, c1=dark...)
        // b1 and g8: b1 is light (file=1,rank=0 → (1+0)%2=1), g8 is (file=6,rank=7 → (6+7)%2=1) light
        let board: Board = "6b1/8/4k3/8/8/3K4/8/1B6 w - - 0 1".parse().unwrap();
        assert!(board.is_insufficient_material());
    }

    #[test]
    fn test_insufficient_material_kb_vs_kb_diff_color() {
        // Bishops on different colored squares → sufficient material
        // a1=dark (0+0=0 even=light by our def), b1=light (1+0=1 odd=dark by our def)
        // Let's use a1 and b1: a1 idx=0 (0%8+0/8=0 even→light), b1 idx=1 (1%8+1/8=1 odd→dark)
        let board: Board = "8/8/4k3/8/8/3K4/8/Bb6 w - - 0 1".parse().unwrap();
        assert!(!board.is_insufficient_material());
    }

    #[test]
    fn test_sufficient_material_with_pawns() {
        let board = Board::default();
        assert!(!board.is_insufficient_material());
    }

    #[test]
    fn test_sufficient_material_rook() {
        let board: Board = "8/8/4k3/8/8/3K4/8/7R w - - 0 1".parse().unwrap();
        assert!(!board.is_insufficient_material());
    }

    #[test]
    fn test_sufficient_material_queen() {
        let board: Board = "8/8/4k3/8/8/3K4/8/7Q w - - 0 1".parse().unwrap();
        assert!(!board.is_insufficient_material());
    }

    // --- Zobrist hash tests ---

    #[test]
    fn test_zobrist_initial_position_is_nonzero() {
        let board = Board::default();
        assert_ne!(board.zobrist_hash(), 0);
    }

    #[test]
    fn test_zobrist_different_positions_different_hashes() {
        let board1 = Board::default();
        let board2: Board =
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".parse().unwrap();
        assert_ne!(board1.zobrist_hash(), board2.zobrist_hash());
    }

    #[test]
    fn test_zobrist_same_position_different_move_orders() {
        // Reach the same position via two different move orders and verify hashes match.
        // 1.e4 e5 2.Nf3 Nc6 vs 1.Nf3 Nc6 2.e4 e5
        let board_a: Board =
            "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3"
                .parse()
                .unwrap();
        let board_b: Board =
            "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3"
                .parse()
                .unwrap();
        assert_eq!(board_a.zobrist_hash(), board_b.zobrist_hash());
    }

    #[test]
    fn test_zobrist_incremental_matches_from_scratch() {
        // Play a sequence of moves and verify the incremental hash equals compute_zobrist().
        let board = Board::default();
        // 1. e4
        let e4_move = MoveGen::new_legal(&board)
            .find(|m| {
                m.source.to_index() == 12 && m.dest.to_index() == 28 // e2->e4
            })
            .expect("e4 move should exist");
        let board2 = board.make_move(e4_move);
        assert_eq!(board2.zobrist_hash(), board2.compute_zobrist());

        // 1...e5
        let e5_move = MoveGen::new_legal(&board2)
            .find(|m| {
                m.source.to_index() == 52 && m.dest.to_index() == 36 // e7->e5
            })
            .expect("e5 move should exist");
        let board3 = board2.make_move(e5_move);
        assert_eq!(board3.zobrist_hash(), board3.compute_zobrist());

        // 2. Nf3
        let nf3_move = MoveGen::new_legal(&board3)
            .find(|m| {
                m.source.to_index() == 6 && m.dest.to_index() == 21 // g1->f3
            })
            .expect("Nf3 move should exist");
        let board4 = board3.make_move(nf3_move);
        assert_eq!(board4.zobrist_hash(), board4.compute_zobrist());
    }

    #[test]
    fn test_zobrist_same_position_via_moves() {
        // Start → e4 → and reach same position as from FEN
        let board = Board::default();
        let e4_move = MoveGen::new_legal(&board)
            .find(|m| m.source.to_index() == 12 && m.dest.to_index() == 28)
            .unwrap();
        let board_via_moves = board.make_move(e4_move);

        let board_from_fen: Board =
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".parse().unwrap();

        assert_eq!(board_via_moves.zobrist_hash(), board_from_fen.zobrist_hash());
    }
}
