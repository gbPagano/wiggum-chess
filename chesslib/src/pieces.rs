use crate::color::Color;
use std::fmt;

/// Represents the different types of chess pieces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl Piece {
    /// Returns the index [0-5] of the piece as a `usize`.
    #[inline(always)]
    pub fn to_index(self) -> usize {
        self as usize
    }

    /// Returns the string representation of the piece, capitalized for white pieces.
    pub fn to_string(self, color: Color) -> String {
        let piece = format!("{}", self);
        if color == Color::White {
            piece.to_uppercase()
        } else {
            piece
        }
    }

    /// Returns the Unicode chess symbol representing the piece.
    pub fn to_symbol(self, color: Color) -> &'static str {
        match (self, color) {
            (Piece::Pawn, Color::White) => "♟",
            (Piece::Pawn, Color::Black) => "♙",
            (Piece::Knight, Color::White) => "♞",
            (Piece::Knight, Color::Black) => "♘",
            (Piece::Bishop, Color::White) => "♝",
            (Piece::Bishop, Color::Black) => "♗",
            (Piece::Rook, Color::White) => "♜",
            (Piece::Rook, Color::Black) => "♖",
            (Piece::Queen, Color::White) => "♛",
            (Piece::Queen, Color::Black) => "♕",
            (Piece::King, Color::White) => "♚",
            (Piece::King, Color::Black) => "♔",
        }
    }
}

impl fmt::Display for Piece {
    /// Formats the piece using standard chess notation (e.g., "p" for pawn, "k" for king).
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Piece::Pawn => "p",
                Piece::Knight => "n",
                Piece::Bishop => "b",
                Piece::Rook => "r",
                Piece::Queen => "q",
                Piece::King => "k",
            }
        )
    }
}

pub const ALL_PIECES: [Piece; 6] = [
    Piece::Pawn,
    Piece::Knight,
    Piece::Bishop,
    Piece::Rook,
    Piece::Queen,
    Piece::King,
];

pub const PROMOTION_PIECES: [Piece; 4] = [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piece_to_index() {
        assert_eq!(Piece::Pawn.to_index(), 0);
        assert_eq!(Piece::Bishop.to_index(), 2);
        assert_eq!(Piece::King.to_index(), 5);
    }

    #[test]
    fn test_piece_to_string() {
        assert_eq!(Piece::Pawn.to_string(Color::Black), "p".to_string());
        assert_eq!(Piece::King.to_string(Color::White), "K".to_string());
    }
}
