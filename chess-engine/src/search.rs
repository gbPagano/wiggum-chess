use chesslib::chess_move::ChessMove;
use chesslib::board::Board;
use chesslib::movegen::MoveGen;

use crate::eval::evaluate;

pub const CHECKMATE_SCORE: i32 = 30000;

/// Negamax search: returns the best (move, score) pair for the side to move.
///
/// Score is from the side-to-move's perspective (positive = good for mover).
/// At depth 0, returns the material evaluation directly (no move).
pub fn search(board: &Board, depth: u8) -> (Option<ChessMove>, i32) {
    if depth == 0 {
        return (None, evaluate(board));
    }

    let moves: Vec<ChessMove> = MoveGen::new_legal(board).collect();

    // Terminal position: checkmate or stalemate
    if moves.is_empty() {
        return (None, evaluate(board));
    }

    let mut best_move = None;
    let mut best_score = i32::MIN + 1; // avoid overflow on negation

    for mv in moves {
        let child = board.make_move(mv);
        let (_, child_score) = search(&child, depth - 1);
        // Negate child score (negamax: good for child = bad for us)
        let score = -child_score;
        if score > best_score {
            best_score = score;
            best_move = Some(mv);
        }
    }

    (best_move, best_score)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn returns_valid_move_from_start() {
        let board = Board::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
            .unwrap();
        let (mv, _score) = search(&board, 1);
        assert!(mv.is_some(), "Expected a legal move from start position");
        // Verify the move is actually legal
        let mv = mv.unwrap();
        let legal_moves: Vec<ChessMove> = MoveGen::new_legal(&board).collect();
        assert!(
            legal_moves.contains(&mv),
            "Returned move is not legal: {:?}",
            mv
        );
    }

    #[test]
    fn finds_mate_in_1() {
        // White: King c7, Queen h1. Black: King a8.
        // Qa1# is checkmate: queen attacks a-file (a8), king can't go to b8 (Kc7) or b7 (Kc7).
        let board = Board::from_str("k7/2K5/8/8/8/8/8/7Q w - - 0 1").unwrap();
        let (mv, score) = search(&board, 1);
        assert!(mv.is_some(), "Expected a move");
        // Score should reflect checkmate found
        assert!(
            score >= CHECKMATE_SCORE - 10,
            "Expected near-checkmate score, got {}",
            score
        );
    }

    #[test]
    fn avoids_hanging_queen() {
        // White has queen on d1, can move to d8 (hanging) or e2 (safe).
        // Black rook on d7 would capture queen if it goes to d8.
        // Position: white Ke1 Qd1, black Ke8 Rd7 — White to move
        // Qd1-d8 is immediately captured by Rd7 (very bad).
        // White should prefer another move.
        // FEN: 4k3/3r4/8/8/8/8/8/3QK3 w - - 0 1
        let board = Board::from_str("4k3/3r4/8/8/8/8/8/3QK3 w - - 0 1").unwrap();
        let (mv, _score) = search(&board, 2);
        assert!(mv.is_some(), "Expected a move");
        let mv = mv.unwrap();
        // The queen should NOT move to d8 (square index: d8 = d file, 8th rank)
        let dest = mv.dest;
        // d8 is file D (index 3), rank 8 (index 7) → square index = 7*8+3 = 59
        assert_ne!(
            dest.to_index(),
            59,
            "Engine should not hang the queen on d8"
        );
    }
}
