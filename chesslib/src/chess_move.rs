use crate::board::Board;
use crate::movegen::MoveGen;
use crate::pieces::Piece;
use crate::square::Square;
use anyhow::{bail, Result};
use std::str::FromStr;

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

    /// Serializes the move to UCI format (e.g. "e2e4", "e7e8q").
    pub fn to_uci(self) -> String {
        match self.promotion {
            None => format!("{}{}", self.source, self.dest),
            Some(p) => format!("{}{}{}", self.source, self.dest, p),
        }
    }

    /// Parses a UCI move string and finds the matching legal move on the board.
    pub fn from_uci(s: &str, board: &Board) -> Result<ChessMove> {
        let len = s.len();
        if len != 4 && len != 5 {
            bail!("invalid UCI move length: {}", s);
        }

        let source = Square::from_str(&s[0..2])?;
        let dest = Square::from_str(&s[2..4])?;
        let promotion = if len == 5 {
            Some(match s.as_bytes()[4] {
                b'q' => Piece::Queen,
                b'r' => Piece::Rook,
                b'b' => Piece::Bishop,
                b'n' => Piece::Knight,
                c => bail!("invalid promotion piece: {}", c as char),
            })
        } else {
            None
        };

        MoveGen::new_legal(board)
            .find(|m| m.source == source && m.dest == dest && m.promotion == promotion)
            .ok_or_else(|| anyhow::anyhow!("illegal move: {}", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;
    use std::str::FromStr;

    fn board(fen: &str) -> Board {
        Board::from_str(fen).unwrap()
    }

    #[test]
    fn test_to_uci_normal_move() {
        let b = board("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        let m = ChessMove::from_uci("e2e4", &b).unwrap();
        assert_eq!(m.to_uci(), "e2e4");
    }

    #[test]
    fn test_to_uci_promotion() {
        // White pawn on e7 ready to promote
        let b = board("8/4P3/8/8/8/8/8/4K2k w - - 0 1");
        let m = ChessMove::from_uci("e7e8q", &b).unwrap();
        assert_eq!(m.to_uci(), "e7e8q");
        assert_eq!(m.promotion, Some(Piece::Queen));
    }

    #[test]
    fn test_to_uci_promotion_all_pieces() {
        let b = board("8/4P3/8/8/8/8/8/4K2k w - - 0 1");
        for (uci, piece) in [("e7e8q", Piece::Queen), ("e7e8r", Piece::Rook),
                              ("e7e8b", Piece::Bishop), ("e7e8n", Piece::Knight)] {
            let m = ChessMove::from_uci(uci, &b).unwrap();
            assert_eq!(m.promotion, Some(piece));
            assert_eq!(m.to_uci(), uci);
        }
    }

    #[test]
    fn test_from_uci_castling() {
        // Kingside castling white
        let b = board("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1");
        let m = ChessMove::from_uci("e1g1", &b).unwrap();
        assert_eq!(m.to_uci(), "e1g1");
        // Queenside castling white
        let m2 = ChessMove::from_uci("e1c1", &b).unwrap();
        assert_eq!(m2.to_uci(), "e1c1");
    }

    #[test]
    fn test_from_uci_en_passant() {
        // White pawn on e5, black pawn just moved to d5 (en passant available)
        let b = board("rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 1");
        let m = ChessMove::from_uci("e5d6", &b).unwrap();
        assert_eq!(m.to_uci(), "e5d6");
    }

    #[test]
    fn test_from_uci_invalid_length() {
        let b = board("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert!(ChessMove::from_uci("e2", &b).is_err());
        assert!(ChessMove::from_uci("e2e4e5x", &b).is_err());
    }

    #[test]
    fn test_from_uci_illegal_move() {
        let b = board("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        // e1e4 is not a legal king move from starting position
        assert!(ChessMove::from_uci("e1e4", &b).is_err());
    }

    #[test]
    fn test_roundtrip() {
        let b = board("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        for m in MoveGen::new_legal(&b) {
            let uci = m.to_uci();
            let parsed = ChessMove::from_uci(&uci, &b).unwrap();
            assert_eq!(m, parsed, "roundtrip failed for {}", uci);
        }
    }
}
