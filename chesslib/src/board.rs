use crate::bitboard::BitBoard;
use crate::castle_rights::CastleRights;
use crate::chess_move::ChessMove;
use crate::color::Color;
use crate::file::{ALL_FILES, File};
use crate::magic;
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
        }
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

        let source_bb = BitBoard::from_square(m.source);
        let dest_bb = BitBoard::from_square(m.dest);

        let moved_piece = self.get_piece(m.source).unwrap();

        result.xor(moved_piece, source_bb, self.side_to_move);
        result.xor(moved_piece, dest_bb, self.side_to_move);
        if let Some(captured) = self.get_piece(m.dest) {
            result.xor(captured, dest_bb, !self.side_to_move);
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
                result.checkers_bitboard ^= magic::get_knight_moves(enemy_king_sq) & dest_bb;
            } else if let Some(promotion) = m.promotion {
                result.xor(Piece::Pawn, dest_bb, self.side_to_move);
                result.xor(promotion, dest_bb, self.side_to_move);
            } else if !(source_bb & magic::get_pawn_source_double_moves()).is_empty()
                && !(dest_bb & magic::get_pawn_dest_double_moves()).is_empty()
            {
                result.set_en_passant(m.dest.backward(self.side_to_move).unwrap());
                result.checkers_bitboard ^=
                    magic::get_pawn_attacks(enemy_king_sq, !self.side_to_move, dest_bb);
            } else if Some(m.dest) == self.en_passant {
                result.xor(
                    Piece::Pawn,
                    BitBoard::from_square(m.dest.forward(!self.side_to_move).unwrap()),
                    !self.side_to_move,
                );
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
            let start_bb = BitBoard::set(
                backrank,
                match m.dest.get_file() {
                    File::C | File::B => File::A,
                    File::G => File::H,
                    _ => unreachable!(),
                },
            );
            let end_bb = BitBoard::set(
                backrank,
                match m.dest.get_file() {
                    File::C | File::B => File::D,
                    File::G => File::F,
                    _ => unreachable!(),
                },
            );
            result.xor(Piece::Rook, start_bb, self.side_to_move);
            result.xor(Piece::Rook, end_bb, self.side_to_move);
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

        result.side_to_move = !result.side_to_move;
        result
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
}
