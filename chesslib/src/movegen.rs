use super::piece_moves::*;
use crate::bitboard::BitBoard;
use crate::board::Board;
use crate::chess_move::ChessMove;
use crate::pieces::PROMOTION_PIECES;
use crate::square::Square;

// use arrayvec::ArrayVec;
use std::iter::ExactSizeIterator;

#[derive(Copy, Clone)]
pub struct BitBoardMove {
    square: Square,
    bitboard: BitBoard,
    promotion: bool,
}

impl BitBoardMove {
    #[inline(always)]
    pub fn new(square: Square, bitboard: BitBoard, promotion: bool) -> Self {
        Self {
            square,
            bitboard,
            promotion,
        }
    }
}

pub type MoveList = Vec<BitBoardMove>;

pub struct MoveGen {
    moves: MoveList,
    promotion_idx: usize,
    idx: usize,
}

impl MoveGen {
    fn enumerate_moves(board: &Board) -> MoveList {
        let checkers = board.get_checkers_bitboard();
        let mask = !board.get_color_bitboard(board.side_to_move());
        //let mut movelist: MoveList = ArrayVec::new();
        let mut movelist = Vec::with_capacity(18);

        if checkers.is_empty() {
            PawnMoves::legals::<NotInCheck>(&mut movelist, board, mask);
            KnightMoves::legals::<NotInCheck>(&mut movelist, board, mask);
            BishopMoves::legals::<NotInCheck>(&mut movelist, board, mask);
            RookMoves::legals::<NotInCheck>(&mut movelist, board, mask);
            QueenMoves::legals::<NotInCheck>(&mut movelist, board, mask);
            KingMoves::legals::<NotInCheck>(&mut movelist, board, mask);
        } else if checkers.0.count_ones() == 1 {
            PawnMoves::legals::<InCheck>(&mut movelist, board, mask);
            KnightMoves::legals::<InCheck>(&mut movelist, board, mask);
            BishopMoves::legals::<InCheck>(&mut movelist, board, mask);
            RookMoves::legals::<InCheck>(&mut movelist, board, mask);
            QueenMoves::legals::<InCheck>(&mut movelist, board, mask);
            KingMoves::legals::<InCheck>(&mut movelist, board, mask);
        } else {
            KingMoves::legals::<InCheck>(&mut movelist, board, mask);
        }

        movelist
    }

    #[inline(always)]
    pub fn new_legal(board: &Board) -> MoveGen {
        MoveGen {
            moves: MoveGen::enumerate_moves(board),
            promotion_idx: 0,
            idx: 0,
        }
    }

    pub fn perft_test(board: &Board, depth: usize) -> usize {
        let movements = MoveGen::new_legal(board);

        let mut result = 0;
        if depth == 1 {
            movements.len()
        } else {
            for m in movements {
                let board = board.make_move(m);
                result += MoveGen::perft_test(&board, depth - 1);
            }
            result
        }
    }
}

impl Iterator for MoveGen {
    type Item = ChessMove;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.moves.len() || self.moves[self.idx].bitboard.is_empty() {
            return None;
        }

        if self.moves[self.idx].promotion {
            let moves = &mut self.moves[self.idx];
            let dest = moves.bitboard.to_square();
            let result = ChessMove::new(
                moves.square,
                dest,
                Some(PROMOTION_PIECES[self.promotion_idx]),
            );
            self.promotion_idx += 1;
            if self.promotion_idx >= PROMOTION_PIECES.len() {
                moves.bitboard ^= BitBoard::from_square(dest);
                self.promotion_idx = 0;
                if moves.bitboard.is_empty() {
                    self.idx += 1;
                }
            }
            return Some(result);
        }

        let moves = &mut self.moves[self.idx];
        let dest = moves.bitboard.to_square();
        moves.bitboard ^= BitBoard::from_square(dest);
        if moves.bitboard.is_empty() {
            self.idx += 1;
        }
        Some(ChessMove::new(moves.square, dest, None))
    }
}

impl ExactSizeIterator for MoveGen {
    fn len(&self) -> usize {
        let mut result = 0;
        for i in 0..self.moves.len() {
            if self.moves[i].promotion {
                result += (self.moves[i].bitboard.0.count_ones() as usize) * PROMOTION_PIECES.len();
            } else {
                result += self.moves[i].bitboard.0.count_ones() as usize;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn movegen_perft_test(fen: &str, depth: usize, result: usize) {
        let board: Board = fen.parse().unwrap();
        assert_eq!(MoveGen::perft_test(&board, depth), result);
    }

    #[test]
    fn movegen_max_movents() {
        let fen = "R6R/3Q4/1Q4Q1/4Q3/2Q4Q/Q4Q2/pp1Q4/kBNN1KB1 w - - 0 1";
        let board: Board = fen.parse().unwrap();
        let movements = MoveGen::new_legal(&board);
        assert_eq!(movements.len(), 218);
    }

    #[test]
    fn movegen_perft() {
        movegen_perft_test("5k2/8/8/8/8/8/8/4K2R w K - 0 1", 6, 661072);
    }

    #[test]
    fn movegen_perft_1() {
        movegen_perft_test("8/5bk1/8/2Pp4/8/1K6/8/8 w - d6 0 1", 6, 824064);
        // Invalid FEN
    }

    #[test]
    fn movegen_perft_2() {
        movegen_perft_test("8/8/1k6/8/2pP4/8/5BK1/8 b - d3 0 1", 6, 824064);
        // Invalid FEN
    }

    #[test]
    fn movegen_perft_3() {
        movegen_perft_test("8/8/1k6/2b5/2pP4/8/5K2/8 b - d3 0 1", 6, 1440467);
    }

    #[test]
    fn movegen_perft_4() {
        movegen_perft_test("8/5k2/8/2Pp4/2B5/1K6/8/8 w - d6 0 1", 6, 1440467);
    }

    #[test]
    fn movegen_perft_5() {
        movegen_perft_test("5k2/8/8/8/8/8/8/4K2R w K - 0 1", 6, 661072);
    }

    #[test]
    fn movegen_perft_6() {
        movegen_perft_test("4k2r/8/8/8/8/8/8/5K2 b k - 0 1", 6, 661072);
    }

    #[test]
    fn movegen_perft_7() {
        movegen_perft_test("3k4/8/8/8/8/8/8/R3K3 w Q - 0 1", 6, 803711);
    }

    #[test]
    fn movegen_perft_8() {
        movegen_perft_test("r3k3/8/8/8/8/8/8/3K4 b q - 0 1", 6, 803711);
    }

    #[test]
    fn movegen_perft_9() {
        movegen_perft_test("r3k2r/1b4bq/8/8/8/8/7B/R3K2R w KQkq - 0 1", 4, 1274206);
    }

    #[test]
    fn movegen_perft_10() {
        movegen_perft_test("r3k2r/7b/8/8/8/8/1B4BQ/R3K2R b KQkq - 0 1", 4, 1274206);
    }

    #[test]
    fn movegen_perft_11() {
        movegen_perft_test("r3k2r/8/3Q4/8/8/5q2/8/R3K2R b KQkq - 0 1", 4, 1720476);
    }

    #[test]
    fn movegen_perft_12() {
        movegen_perft_test("r3k2r/8/5Q2/8/8/3q4/8/R3K2R w KQkq - 0 1", 4, 1720476);
    }

    #[test]
    fn movegen_perft_13() {
        movegen_perft_test("2K2r2/4P3/8/8/8/8/8/3k4 w - - 0 1", 6, 3821001);
    }

    #[test]
    fn movegen_perft_14() {
        movegen_perft_test("3K4/8/8/8/8/8/4p3/2k2R2 b - - 0 1", 6, 3821001);
    }

    #[test]
    fn movegen_perft_15() {
        movegen_perft_test("8/8/1P2K3/8/2n5/1q6/8/5k2 b - - 0 1", 5, 1004658);
    }

    #[test]
    fn movegen_perft_16() {
        movegen_perft_test("5K2/8/1Q6/2N5/8/1p2k3/8/8 w - - 0 1", 5, 1004658);
    }

    #[test]
    fn movegen_perft_17() {
        movegen_perft_test("4k3/1P6/8/8/8/8/K7/8 w - - 0 1", 6, 217342);
    }

    #[test]
    fn movegen_perft_18() {
        movegen_perft_test("8/k7/8/8/8/8/1p6/4K3 b - - 0 1", 6, 217342);
    }

    #[test]
    fn movegen_perft_19() {
        movegen_perft_test("8/P1k5/K7/8/8/8/8/8 w - - 0 1", 6, 92683);
    }

    #[test]
    fn movegen_perft_20() {
        movegen_perft_test("8/8/8/8/8/k7/p1K5/8 b - - 0 1", 6, 92683);
    }

    #[test]
    fn movegen_perft_21() {
        movegen_perft_test("K1k5/8/P7/8/8/8/8/8 w - - 0 1", 6, 2217);
    }

    #[test]
    fn movegen_perft_22() {
        movegen_perft_test("8/8/8/8/8/p7/8/k1K5 b - - 0 1", 6, 2217);
    }

    #[test]
    fn movegen_perft_23() {
        movegen_perft_test("8/k1P5/8/1K6/8/8/8/8 w - - 0 1", 7, 567584);
    }

    #[test]
    fn movegen_perft_24() {
        movegen_perft_test("8/8/8/8/1k6/8/K1p5/8 b - - 0 1", 7, 567584);
    }

    #[test]
    fn movegen_perft_25() {
        movegen_perft_test("8/8/2k5/5q2/5n2/8/5K2/8 b - - 0 1", 4, 23527);
    }

    #[test]
    fn movegen_perft_26() {
        movegen_perft_test("8/5k2/8/5N2/5Q2/2K5/8/8 w - - 0 1", 4, 23527);
    }
}
