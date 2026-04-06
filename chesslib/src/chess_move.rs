use crate::pieces::Piece;
use crate::square::Square;

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct ChessMove {
    pub source: Square,
    pub dest: Square,
    pub promotion: Option<Piece>,
}

impl ChessMove {
    #[inline(always)]
    pub fn new(source: Square, dest: Square, promotion: Option<Piece>) -> ChessMove {
        ChessMove {
            source,
            dest,
            promotion,
        }
    }
}
