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
}
