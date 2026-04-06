use crate::color::Color;
use crate::file::File;
use crate::rank::Rank;
use anyhow::{Error, bail};
use std::fmt;
use std::str::FromStr;

/// Represents a square on a chessboard, identified by a rank and file.
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Square(u8);

impl Square {
    #[inline(always)]
    pub const fn new(rank: Rank, file: File) -> Self {
        Square(((rank.to_index() << 3) ^ file.to_index()) as u8)
    }

    /// Creates a `Square` from an index (0-63), ensuring it remains within bounds.
    #[inline(always)]
    pub const fn from_index(idx: u8) -> Self {
        Square(idx & 63)
    }

    #[inline(always)]
    pub const fn to_index(self) -> usize {
        self.0 as usize
    }

    #[inline(always)]
    pub const fn get_rank(&self) -> Rank {
        Rank::from_index((self.0 >> 3) as usize)
    }

    #[inline(always)]
    pub const fn get_file(&self) -> File {
        File::from_index((self.0 & 7) as usize)
    }

    /// Returns the square one rank above, if possible.
    #[inline(always)]
    pub fn up(&self) -> Option<Square> {
        if self.get_rank() == Rank::Eighth {
            None
        } else {
            Some(Square::new(self.get_rank().up(), self.get_file()))
        }
    }

    /// Returns the square one rank below, if possible.
    #[inline(always)]
    pub fn down(&self) -> Option<Square> {
        if self.get_rank() == Rank::First {
            None
        } else {
            Some(Square::new(self.get_rank().down(), self.get_file()))
        }
    }

    /// Returns the square one file to the left, if possible.
    #[inline(always)]
    pub fn left(&self) -> Option<Square> {
        if self.get_file() == File::A {
            None
        } else {
            Some(Square::new(self.get_rank(), self.get_file().left()))
        }
    }

    /// Returns the square one file to the right, if possible.
    #[inline(always)]
    pub fn right(&self) -> Option<Square> {
        if self.get_file() == File::H {
            None
        } else {
            Some(Square::new(self.get_rank(), self.get_file().right()))
        }
    }

    #[inline(always)]
    pub fn forward(&self, color: Color) -> Option<Square> {
        match color {
            Color::White => self.up(),
            Color::Black => self.down(),
        }
    }

    #[inline(always)]
    pub fn backward(&self, color: Color) -> Option<Square> {
        match color {
            Color::White => self.down(),
            Color::Black => self.up(),
        }
    }

    /// Checks if the square is on the edge of the board
    #[inline(always)]
    pub fn is_edge(&self) -> bool {
        self.get_file().is_edge() || self.get_rank().is_edge()
    }

    /// Returns an iterator over all 64 squares on the board.
    #[inline(always)]
    pub fn all_squares() -> impl Iterator<Item = Square> {
        (0..64).map(Square::from_index)
    }
}


impl Default for Square {
    /// Returns the default square (A1).
    #[inline(always)]
    fn default() -> Self {
        Self::new(Rank::First, File::A)
    }
}

impl FromStr for Square {
    type Err = Error;

    /// Parses a square from a string representation (e.g., "a1").
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 2 {
            bail!("error");
        }

        let mut chars = s.chars();
        let file_char = chars.next().unwrap();
        let rank_char = chars.next().unwrap();

        let file = match file_char {
            'a'..='h' => File::from_index(file_char as usize - 'a' as usize),
            _ => bail!("error"),
        };

        let rank = match rank_char.to_digit(10) {
            Some(n @ 1..=8) => Rank::from_index((n - 1) as usize),
            _ => bail!("error"),
        };

        Ok(Square::new(rank, file))
    }
}

impl fmt::Display for Square {
    /// Formats the square as a standard chess notation string (e.g., "a1").
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}{}",
            (b'a' + (self.0 & 7)) as char,
            (b'1' + (self.0 >> 3)) as char
        )
    }
}

impl fmt::Debug for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{}", self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_square() {
        assert_eq!(Square::new(Rank::First, File::A), Square::from_index(0));
        assert_eq!(Square::new(Rank::Third, File::C), Square::from_index(18));
        assert_eq!(Square::new(Rank::Seventh, File::G), Square::from_index(54));
    }

    #[test]
    fn test_rank_and_file_from_square() {
        assert_eq!(Square::new(Rank::First, File::A).get_rank(), Rank::First);
        assert_eq!(Square::new(Rank::Seventh, File::G).get_file(), File::G);
    }

    #[test]
    fn test_rank_from_str() {
        assert_eq!(
            Square::from_str("a1").unwrap(),
            Square::new(Rank::First, File::A)
        );
        assert_eq!(
            Square::from_str("e3").unwrap(),
            Square::new(Rank::Third, File::E)
        );
    }

    #[test]
    fn test_rank_fmt() {
        assert_eq!(format!("{}", Square::new(Rank::First, File::A)), "a1");
        assert_eq!(format!("{}", Square::new(Rank::Third, File::E)), "e3");
    }
}
