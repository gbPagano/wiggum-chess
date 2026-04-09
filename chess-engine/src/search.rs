use chesslib::board::Board;
use chesslib::chess_move::ChessMove;
use chesslib::movegen::MoveGen;

use crate::eval::evaluate;

pub const CHECKMATE_SCORE: i32 = 30000;
const INF: i32 = CHECKMATE_SCORE + 1_000;

/// Runs a negamax search with alpha-beta pruning and returns the best result
/// for the side to move.
///
/// The returned score is from the side-to-move's perspective
/// (positive = good for the player to move). At depth 0 and in terminal
/// positions, no move is returned and the static evaluation is used.
pub fn search(board: &Board, depth: u8) -> (Option<ChessMove>, i32) {
    alpha_beta(board, depth, -INF, INF)
}

/// Searches a position with negamax alpha-beta pruning inside the window
/// `[alpha, beta]` and returns the best legal move found together with its
/// score from the current side-to-move's perspective.
///
/// This implementation is used both at the root and in recursive child nodes.
/// In recursive calls, the move from deeper levels is not propagated upward;
/// each caller keeps the current move that produced the best child score.
fn alpha_beta(board: &Board, depth: u8, mut alpha: i32, beta: i32) -> (Option<ChessMove>, i32) {
    if depth == 0 {
        return (None, evaluate(board));
    }

    let moves: Vec<ChessMove> = MoveGen::new_legal(board).collect();

    // Terminal position: checkmate or stalemate
    if moves.is_empty() {
        return (None, evaluate(board));
    }

    let mut best_move = None;
    let mut best_score = -INF;

    for mv in moves {
        let child = board.make_move(mv);
        let (_child_move, child_score) = alpha_beta(&child, depth - 1, -beta, -alpha);
        let score = -child_score;

        if score > best_score {
            best_score = score;
            best_move = Some(mv);
        }

        alpha = alpha.max(score);
        if alpha >= beta {
            break;
        }
    }

    (best_move, best_score)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn returns_static_evaluation_at_depth_zero() {
        let board =
            Board::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let (mv, score) = search(&board, 0);
        assert!(mv.is_none(), "Expected no move at depth 0");
        assert_eq!(score, evaluate(&board));
    }

    #[test]
    fn returns_valid_move_from_start() {
        let board =
            Board::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
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
        // White: King c7, Queen b1. Black: King a8.
        let board = Board::from_str("k7/2K5/8/8/8/8/8/1Q6 w - - 0 1").unwrap();
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

    #[test]
    fn returns_checkmate_score_without_move_in_terminal_position() {
        let board = Board::from_str("7k/6Q1/6K1/8/8/8/8/8 b - - 0 1").unwrap();
        let (mv, score) = search(&board, 3);
        assert!(mv.is_none(), "Expected no move in checkmate");
        assert_eq!(score, -CHECKMATE_SCORE);
    }

    #[test]
    fn terminal_positions_are_evaluated_without_searching_children() {
        let board = Board::from_str("k7/2K5/1Q6/8/8/8/8/8 b - - 0 1").unwrap();
        let (mv, score) = search(&board, 3);
        assert!(mv.is_none(), "Expected no move in stalemate");
        assert_eq!(score, 0);
    }

    #[test]
    fn returns_only_legal_move_when_forced() {
        let board = Board::from_str("7k/8/p5Q1/8/8/8/8/5K2 b - - 0 1").unwrap();
        let legal_moves: Vec<ChessMove> = MoveGen::new_legal(&board).collect();
        assert_eq!(legal_moves.len(), 1, "Expected exactly one legal move");

        let (mv, _score) = search(&board, 2);
        assert_eq!(mv, Some(legal_moves[0]));
    }
}
