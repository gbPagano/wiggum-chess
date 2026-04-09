/// Color of a chess piece or side to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn opponent(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

/// Kind of chess piece, independent of color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

/// A colored chess piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub kind: PieceKind,
    pub color: Color,
}

impl Piece {
    pub fn new(kind: PieceKind, color: Color) -> Self {
        Self { kind, color }
    }
}

/// Castling rights: which sides each color can still castle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastlingRights {
    pub white_kingside: bool,
    pub white_queenside: bool,
    pub black_kingside: bool,
    pub black_queenside: bool,
}

impl CastlingRights {
    pub fn all() -> Self {
        Self {
            white_kingside: true,
            white_queenside: true,
            black_kingside: true,
            black_queenside: true,
        }
    }

    pub fn none() -> Self {
        Self {
            white_kingside: false,
            white_queenside: false,
            black_kingside: false,
            black_queenside: false,
        }
    }
}

/// A square on the board addressed as (rank, file) where both are 0-based.
/// rank 0 = rank 1 (white's back rank), rank 7 = rank 8 (black's back rank).
/// file 0 = file a, file 7 = file h.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Square {
    pub rank: u8,
    pub file: u8,
}

impl Square {
    pub fn new(rank: u8, file: u8) -> Self {
        debug_assert!(rank < 8 && file < 8, "Square out of bounds");
        Self { rank, file }
    }
}

/// A chess move from one square to another.
///
/// For now this represents normal moves and captures.
/// Special moves (promotion, castling, en passant) will extend this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub from: Square,
    pub to: Square,
}

impl Move {
    pub fn new(from: Square, to: Square) -> Self {
        Self { from, to }
    }
}

/// The full chess board state.
///
/// The board is stored as an 8×8 matrix of `Option<Piece>`.
/// Index: `squares[rank][file]`, rank 0 = rank 1 (white's home rank).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub squares: [[Option<Piece>; 8]; 8],
    pub side_to_move: Color,
    pub castling: CastlingRights,
    /// If `Some(file)`, the file on which an en passant capture is possible.
    /// The rank is always the 6th rank from white's perspective (rank index 5 for white, 2 for black).
    pub en_passant_file: Option<u8>,
    pub halfmove_clock: u32,
    pub fullmove_number: u32,
}

impl Board {
    /// Returns an empty board (no pieces, white to move, no rights).
    pub fn empty() -> Self {
        Self {
            squares: [[None; 8]; 8],
            side_to_move: Color::White,
            castling: CastlingRights::none(),
            en_passant_file: None,
            halfmove_clock: 0,
            fullmove_number: 1,
        }
    }

    /// Returns the standard starting position.
    pub fn starting_position() -> Self {
        let mut b = Self::empty();
        b.castling = CastlingRights::all();

        use PieceKind::*;
        let back_rank = [Rook, Knight, Bishop, Queen, King, Bishop, Knight, Rook];

        for (file, &kind) in back_rank.iter().enumerate() {
            b.squares[0][file] = Some(Piece::new(kind, Color::White));
            b.squares[7][file] = Some(Piece::new(kind, Color::Black));
        }
        for file in 0..8 {
            b.squares[1][file] = Some(Piece::new(Pawn, Color::White));
            b.squares[6][file] = Some(Piece::new(Pawn, Color::Black));
        }

        b
    }

    /// Apply a normal move or capture, returning the resulting board.
    ///
    /// The piece at `mv.from` is moved to `mv.to`. Any piece previously on
    /// `mv.to` is removed (capture). The side to move is toggled.
    ///
    /// Caller is responsible for passing a legal move; this method does not
    /// validate legality.
    pub fn apply_move(&self, mv: Move) -> Board {
        let mut next = self.clone();

        let piece = next.squares[mv.from.rank as usize][mv.from.file as usize]
            .take()
            .expect("apply_move: no piece on from-square");

        let captured = next.squares[mv.to.rank as usize][mv.to.file as usize].replace(piece);

        // Halfmove clock: reset on pawn move or capture, else increment.
        if piece.kind == PieceKind::Pawn || captured.is_some() {
            next.halfmove_clock = 0;
        } else {
            next.halfmove_clock += 1;
        }

        // Fullmove number increments after black's move.
        if self.side_to_move == Color::Black {
            next.fullmove_number += 1;
        }

        // Set en passant file if this is a pawn double push, else clear it.
        next.en_passant_file = if piece.kind == PieceKind::Pawn {
            match self.side_to_move {
                Color::White if mv.from.rank == 1 && mv.to.rank == 3 => Some(mv.from.file),
                Color::Black if mv.from.rank == 6 && mv.to.rank == 4 => Some(mv.from.file),
                _ => None,
            }
        } else {
            None
        };

        next.side_to_move = self.side_to_move.opponent();
        next
    }

    /// Generate pseudo-legal pawn moves for the side to move.
    ///
    /// Includes single pushes, double pushes from the starting rank, and
    /// diagonal captures of opposing pieces. Does not include en passant or
    /// promotion (those are covered in later stories).
    pub fn pseudo_legal_pawn_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();
        let color = self.side_to_move;

        // Direction pawns advance: +1 rank for white, -1 rank for black.
        let (start_rank, advance_dir, promote_rank): (u8, i8, u8) = match color {
            Color::White => (1, 1, 7),
            Color::Black => (6, -1, 0),
        };

        for rank in 0..8u8 {
            for file in 0..8u8 {
                let sq = Square::new(rank, file);
                let Some(piece) = self.get(sq) else { continue };
                if piece.kind != PieceKind::Pawn || piece.color != color {
                    continue;
                }

                // Single push
                let to_rank = rank as i8 + advance_dir;
                if to_rank < 0 || to_rank > 7 {
                    continue;
                }
                let to_rank = to_rank as u8;

                // Skip promotion squares — promotion is handled in a later story.
                if to_rank == promote_rank {
                    continue;
                }

                if self.squares[to_rank as usize][file as usize].is_none() {
                    moves.push(Move::new(sq, Square::new(to_rank, file)));

                    // Double push from starting rank
                    if rank == start_rank {
                        let to_rank2 = (to_rank as i8 + advance_dir) as u8;
                        if self.squares[to_rank2 as usize][file as usize].is_none() {
                            moves.push(Move::new(sq, Square::new(to_rank2, file)));
                        }
                    }
                }

                // Diagonal captures
                for &df in &[-1i8, 1i8] {
                    let cap_file = file as i8 + df;
                    if cap_file < 0 || cap_file > 7 {
                        continue;
                    }
                    let cap_file = cap_file as u8;
                    let cap_sq = Square::new(to_rank, cap_file);
                    if let Some(target) = self.get(cap_sq) {
                        if target.color != color {
                            // Skip captures that would land on the promotion rank.
                            if to_rank != promote_rank {
                                moves.push(Move::new(sq, cap_sq));
                            }
                        }
                    }
                }
            }
        }

        moves
    }

    /// Get the piece at a square (if any).
    pub fn get(&self, sq: Square) -> Option<Piece> {
        self.squares[sq.rank as usize][sq.file as usize]
    }

    /// Set the piece at a square.
    pub fn set(&mut self, sq: Square, piece: Option<Piece>) {
        self.squares[sq.rank as usize][sq.file as usize] = piece;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_board_has_no_pieces() {
        let board = Board::empty();
        for rank in 0..8u8 {
            for file in 0..8u8 {
                assert!(board.get(Square::new(rank, file)).is_none());
            }
        }
    }

    #[test]
    fn starting_position_has_correct_pieces() {
        let board = Board::starting_position();

        // White back rank
        assert_eq!(
            board.get(Square::new(0, 0)),
            Some(Piece::new(PieceKind::Rook, Color::White))
        );
        assert_eq!(
            board.get(Square::new(0, 4)),
            Some(Piece::new(PieceKind::King, Color::White))
        );
        // Black back rank
        assert_eq!(
            board.get(Square::new(7, 4)),
            Some(Piece::new(PieceKind::King, Color::Black))
        );
        // Pawns
        assert_eq!(
            board.get(Square::new(1, 3)),
            Some(Piece::new(PieceKind::Pawn, Color::White))
        );
        assert_eq!(
            board.get(Square::new(6, 3)),
            Some(Piece::new(PieceKind::Pawn, Color::Black))
        );
        // Empty middle
        assert!(board.get(Square::new(3, 3)).is_none());
    }

    #[test]
    fn starting_position_metadata() {
        let board = Board::starting_position();
        assert_eq!(board.side_to_move, Color::White);
        assert!(board.castling.white_kingside);
        assert!(board.castling.white_queenside);
        assert!(board.castling.black_kingside);
        assert!(board.castling.black_queenside);
        assert!(board.en_passant_file.is_none());
    }

    #[test]
    fn color_opponent() {
        assert_eq!(Color::White.opponent(), Color::Black);
        assert_eq!(Color::Black.opponent(), Color::White);
    }

    // --- US-003: move application tests ---

    #[test]
    fn normal_move_updates_squares_and_side_to_move() {
        let board = Board::starting_position();
        // Move white pawn e2→e3 (rank 1, file 4 → rank 2, file 4)
        let from = Square::new(1, 4);
        let to = Square::new(2, 4);
        let after = board.apply_move(Move::new(from, to));

        assert!(after.get(from).is_none(), "from-square should be empty");
        assert_eq!(
            after.get(to),
            Some(Piece::new(PieceKind::Pawn, Color::White)),
            "to-square should have white pawn"
        );
        assert_eq!(after.side_to_move, Color::Black, "side to move should switch");
    }

    #[test]
    fn capture_removes_captured_piece() {
        // Place a white rook on e4 and a black pawn on e5, then capture.
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        let attacker_sq = Square::new(3, 4); // e4
        let target_sq = Square::new(4, 4);   // e5
        board.set(attacker_sq, Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(target_sq, Some(Piece::new(PieceKind::Pawn, Color::Black)));

        let after = board.apply_move(Move::new(attacker_sq, target_sq));

        assert!(after.get(attacker_sq).is_none(), "rook should have left e4");
        assert_eq!(
            after.get(target_sq),
            Some(Piece::new(PieceKind::Rook, Color::White)),
            "rook should be on e5; captured pawn should be gone"
        );
        assert_eq!(after.side_to_move, Color::Black);
    }

    #[test]
    fn fullmove_increments_after_black_move() {
        let mut board = Board::empty();
        board.side_to_move = Color::Black;
        board.fullmove_number = 3;
        board.set(Square::new(6, 0), Some(Piece::new(PieceKind::Rook, Color::Black)));

        let after = board.apply_move(Move::new(Square::new(6, 0), Square::new(5, 0)));
        assert_eq!(after.fullmove_number, 4);
    }

    #[test]
    fn halfmove_clock_resets_on_capture() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.halfmove_clock = 10;
        board.set(Square::new(0, 0), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(4, 0), Some(Piece::new(PieceKind::Pawn, Color::Black)));

        let after = board.apply_move(Move::new(Square::new(0, 0), Square::new(4, 0)));
        assert_eq!(after.halfmove_clock, 0);
    }

    #[test]
    fn halfmove_clock_increments_on_quiet_non_pawn_move() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.halfmove_clock = 5;
        board.set(Square::new(0, 0), Some(Piece::new(PieceKind::Rook, Color::White)));

        let after = board.apply_move(Move::new(Square::new(0, 0), Square::new(4, 0)));
        assert_eq!(after.halfmove_clock, 6);
    }

    // --- US-004: pawn pseudo-legal move generation tests ---

    #[test]
    fn white_pawn_single_push() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        // White pawn on e4 (rank 3, file 4) — not on start rank
        board.set(Square::new(3, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(moves.contains(&Move::new(Square::new(3, 4), Square::new(4, 4))));
        // Should not include a double push (not on start rank)
        assert!(!moves.contains(&Move::new(Square::new(3, 4), Square::new(5, 4))));
    }

    #[test]
    fn white_pawn_double_push_from_start_rank() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        // White pawn on e2 (rank 1, file 4)
        board.set(Square::new(1, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(moves.contains(&Move::new(Square::new(1, 4), Square::new(2, 4))));
        assert!(moves.contains(&Move::new(Square::new(1, 4), Square::new(3, 4))));
    }

    #[test]
    fn white_pawn_double_push_blocked_by_piece() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(1, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));
        // Block rank 2
        board.set(Square::new(2, 4), Some(Piece::new(PieceKind::Rook, Color::Black)));
        let moves = board.pseudo_legal_pawn_moves();
        // Neither single nor double push should be generated
        assert!(!moves.contains(&Move::new(Square::new(1, 4), Square::new(2, 4))));
        assert!(!moves.contains(&Move::new(Square::new(1, 4), Square::new(3, 4))));
    }

    #[test]
    fn white_pawn_double_push_blocked_by_second_piece() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(1, 4), Some(Piece::new(PieceKind::Pawn, Color::White)));
        // Block rank 3 (the double-push target)
        board.set(Square::new(3, 4), Some(Piece::new(PieceKind::Rook, Color::Black)));
        let moves = board.pseudo_legal_pawn_moves();
        // Single push allowed but not double
        assert!(moves.contains(&Move::new(Square::new(1, 4), Square::new(2, 4))));
        assert!(!moves.contains(&Move::new(Square::new(1, 4), Square::new(3, 4))));
    }

    #[test]
    fn white_pawn_captures_diagonally() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        // White pawn on d4
        board.set(Square::new(3, 3), Some(Piece::new(PieceKind::Pawn, Color::White)));
        // Black pieces on c5 and e5
        board.set(Square::new(4, 2), Some(Piece::new(PieceKind::Pawn, Color::Black)));
        board.set(Square::new(4, 4), Some(Piece::new(PieceKind::Knight, Color::Black)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(moves.contains(&Move::new(Square::new(3, 3), Square::new(4, 2))));
        assert!(moves.contains(&Move::new(Square::new(3, 3), Square::new(4, 4))));
    }

    #[test]
    fn white_pawn_cannot_capture_friendly() {
        let mut board = Board::empty();
        board.side_to_move = Color::White;
        board.set(Square::new(3, 3), Some(Piece::new(PieceKind::Pawn, Color::White)));
        // White pieces on both capture diagonals
        board.set(Square::new(4, 2), Some(Piece::new(PieceKind::Rook, Color::White)));
        board.set(Square::new(4, 4), Some(Piece::new(PieceKind::Rook, Color::White)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(!moves.contains(&Move::new(Square::new(3, 3), Square::new(4, 2))));
        assert!(!moves.contains(&Move::new(Square::new(3, 3), Square::new(4, 4))));
    }

    #[test]
    fn black_pawn_single_push() {
        let mut board = Board::empty();
        board.side_to_move = Color::Black;
        // Black pawn on e5 (rank 4, file 4)
        board.set(Square::new(4, 4), Some(Piece::new(PieceKind::Pawn, Color::Black)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(moves.contains(&Move::new(Square::new(4, 4), Square::new(3, 4))));
    }

    #[test]
    fn black_pawn_double_push_from_start_rank() {
        let mut board = Board::empty();
        board.side_to_move = Color::Black;
        // Black pawn on e7 (rank 6, file 4)
        board.set(Square::new(6, 4), Some(Piece::new(PieceKind::Pawn, Color::Black)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(moves.contains(&Move::new(Square::new(6, 4), Square::new(5, 4))));
        assert!(moves.contains(&Move::new(Square::new(6, 4), Square::new(4, 4))));
    }

    #[test]
    fn black_pawn_captures_diagonally() {
        let mut board = Board::empty();
        board.side_to_move = Color::Black;
        // Black pawn on d5
        board.set(Square::new(4, 3), Some(Piece::new(PieceKind::Pawn, Color::Black)));
        // White pieces on c4 and e4
        board.set(Square::new(3, 2), Some(Piece::new(PieceKind::Pawn, Color::White)));
        board.set(Square::new(3, 4), Some(Piece::new(PieceKind::Knight, Color::White)));
        let moves = board.pseudo_legal_pawn_moves();
        assert!(moves.contains(&Move::new(Square::new(4, 3), Square::new(3, 2))));
        assert!(moves.contains(&Move::new(Square::new(4, 3), Square::new(3, 4))));
    }

    #[test]
    fn apply_move_sets_en_passant_file_on_double_push() {
        let board = Board::starting_position();
        // White pawn e2→e4 double push
        let after = board.apply_move(Move::new(Square::new(1, 4), Square::new(3, 4)));
        assert_eq!(after.en_passant_file, Some(4));
    }

    #[test]
    fn apply_move_clears_en_passant_on_single_push() {
        let board = Board::starting_position();
        // White pawn e2→e3 single push
        let after = board.apply_move(Move::new(Square::new(1, 4), Square::new(2, 4)));
        assert_eq!(after.en_passant_file, None);
    }
}
