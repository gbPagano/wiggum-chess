use crate::file::File;
use crate::rank::Rank;
use crate::square::Square;

use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Mul, Not, Shr};

/// Represents a 64-bit bitboard, where each bit corresponds to a square on a chessboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitBoard(pub u64);

impl BitAnd for BitBoard {
    type Output = Self;
    #[inline(always)]
    fn bitand(self, rhs: Self) -> Self::Output {
        BitBoard(self.0 & rhs.0)
    }
}

impl BitOr for BitBoard {
    type Output = Self;
    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self::Output {
        BitBoard(self.0 | rhs.0)
    }
}

impl BitXor for BitBoard {
    type Output = Self;
    #[inline(always)]
    fn bitxor(self, rhs: Self) -> Self::Output {
        BitBoard(self.0 ^ rhs.0)
    }
}

impl Not for BitBoard {
    type Output = Self;
    #[inline(always)]
    fn not(self) -> Self::Output {
        BitBoard(!self.0)
    }
}

impl BitAndAssign for BitBoard {
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitOrAssign for BitBoard {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitXorAssign for BitBoard {
    #[inline(always)]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl Mul for BitBoard {
    type Output = BitBoard;

    #[inline(always)]
    fn mul(self, rhs: BitBoard) -> BitBoard {
        BitBoard(self.0.wrapping_mul(rhs.0))
    }
}

impl Shr<u8> for BitBoard {
    type Output = Self;

    #[inline(always)]
    fn shr(self, rhs: u8) -> Self::Output {
        BitBoard(self.0 >> rhs)
    }
}

impl BitBoard {
    #[inline(always)]
    pub fn new(val: u64) -> BitBoard {
        BitBoard(val)
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Converts a `Square` into a `BitBoard` with a single bit set corresponding to the square.
    #[inline(always)]
    pub fn from_square(tile: Square) -> BitBoard {
        BitBoard(1u64 << tile.to_index())
    }

    /// Convert a `BitBoard` to a `Square`. Returns the least-significant `Square`
    #[inline(always)]
    pub fn to_square(self) -> Square {
        Square::from_index(self.0.trailing_zeros() as u8)
    }

    /// Creates a `BitBoard` from a specific rank and file.
    #[inline(always)]
    pub fn set(rank: Rank, file: File) -> BitBoard {
        BitBoard::from_square(Square::new(rank, file))
    }

    /// Returns a `Iterator<Square>` containing all the squares that are set in the `BitBoard`.
    #[inline(always)]
    pub fn get_squares(&self) -> impl Iterator<Item = Square> + use<> {
        let mut bb = self.0;

        std::iter::from_fn(move || {
            if bb == 0 {
                None
            } else {
                let idx = bb.trailing_zeros() as u8;
                bb &= bb - 1;
                Some(Square::from_index(idx))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitboard_bitwise_ops() {
        let a = BitBoard(0b1100);
        let b = BitBoard(0b1010);

        assert_eq!(a & b, BitBoard(0b1000));
        assert_eq!(a | b, BitBoard(0b1110));
        assert_eq!(a ^ b, BitBoard(0b0110));
        assert_eq!(!a, BitBoard(!0b1100));
    }

    #[test]
    fn test_bitboard_from_square() {
        let tile = Square::new(Rank::First, File::H);
        assert_eq!(BitBoard::from_square(tile), BitBoard::new(0b10000000));
    }
}
