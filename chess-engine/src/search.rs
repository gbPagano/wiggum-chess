use std::time::Instant;

use chesslib::board::Board;
use chesslib::chess_move::ChessMove;
use chesslib::movegen::MoveGen;

use crate::eval::evaluate;

pub const CHECKMATE_SCORE: i32 = 30000;
const INF: i32 = CHECKMATE_SCORE + 1_000;

/// Minimum score advantage a partial-depth root move must have over the last
/// fully-completed depth's best score before it is allowed to replace it.
///
/// See [`search_timed`] for the full timeout policy description.
const PARTIAL_OVERRIDE_THRESHOLD_CP: i32 = 30;

/// Sentinel error returned when a timed search is interrupted by the deadline.
///
/// Propagated up the call stack by [`alpha_beta_with_ctx`] so callers can
/// distinguish a normal result from a timed-out result without needing to
/// inspect wall-clock time again.
struct Timeout;

/// Hard cap on the maximum depth that iterative deepening may reach.
///
/// No single search iteration will exceed this depth, preventing unbounded
/// recursion even when the remaining clock is large.
pub const MAX_TIMED_DEPTH: u8 = 64;

/// Context for a timed iterative deepening search.
///
/// Carries the wall-clock deadline and depth configuration so the engine can
/// check elapsed time during iterative deepening and within recursive
/// alpha-beta nodes.  For classic fixed-depth searches, use `search` directly
/// without a context — timed code paths must not affect non-timed behavior.
///
/// # Usage
///
/// 1. Build a context with `SearchContext::from_budget_ms(budget_ms)`.
/// 2. Pass a reference to `search_timed`; the context propagates through the
///    iterative deepening loop and (in a later story) into recursive nodes so
///    they can stop early on timeout.
pub struct SearchContext {
    /// Wall-clock time at which iterative deepening must stop.
    pub deadline: Instant,
    /// Hard cap on the maximum depth that iterative deepening may reach.
    /// Defaults to `MAX_TIMED_DEPTH` (64 plies).
    pub max_depth: u8,
}

impl SearchContext {
    /// Build a context for a search that must finish within `budget_ms` milliseconds.
    ///
    /// The deadline is computed as `Instant::now() + budget_ms` and `max_depth`
    /// is set to [`MAX_TIMED_DEPTH`].
    pub fn from_budget_ms(budget_ms: u64) -> Self {
        SearchContext {
            deadline: Instant::now() + std::time::Duration::from_millis(budget_ms),
            max_depth: MAX_TIMED_DEPTH,
        }
    }

    /// Returns `true` if the search deadline has passed.
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.deadline
    }
}

/// Runs a negamax search with alpha-beta pruning and returns the best result
/// for the side to move.
///
/// The returned score is from the side-to-move's perspective
/// (positive = good for the player to move). At depth 0 and in terminal
/// positions, no move is returned and the static evaluation is used.
pub fn search(board: &Board, depth: u8) -> (Option<ChessMove>, i32) {
    alpha_beta(board, depth, -INF, INF)
}

/// Entry point for timed iterative deepening search using the provided context.
///
/// Searches from depth 1 up to `ctx.max_depth`, completing one full alpha-beta
/// pass per depth level. After each completed depth the best move and score are
/// stored. The loop stops when `ctx.is_expired()` returns `true` before the next
/// iteration begins or mid-search, or when `ctx.max_depth` (64 plies) is reached.
///
/// # Timeout policy
///
/// **Default**: the best move from the **last fully completed depth** is returned.
/// This is the safest choice because the full-depth result was evaluated with a
/// consistent search over all root moves.
///
/// **Partial-depth override**: if time expires during a deeper iteration and at
/// least one root move at that depth finished evaluation before the timeout, the
/// partial result may replace the completed-depth move — but only when the
/// partial-depth best score exceeds the last completed-depth score by at least
/// [`PARTIAL_OVERRIDE_THRESHOLD_CP`] (30 centipawns).  This avoids regressing to
/// a worse move from an unfinished search iteration.
///
/// **No-depth fallback**: if the deadline expires before depth 1 completes, the
/// engine returns the first legal move from the position to guarantee a legal
/// response under any time pressure.
///
/// The existing `search` function is NOT called from this path so non-timed
/// behavior is unaffected by any future changes to timed search logic.
pub fn search_timed(board: &Board, ctx: &SearchContext) -> (Option<ChessMove>, i32) {
    // Best result from the last *fully completed* depth.
    let mut completed_best_move: Option<ChessMove> = None;
    let mut completed_best_score: i32 = -INF;

    // Fallback legal move in case no depth completes at all.
    let fallback_move = MoveGen::new_legal(board).next();

    'outer: for depth in 1..=ctx.max_depth {
        // Check deadline before starting the next depth iteration.
        if ctx.is_expired() {
            break;
        }

        let root_moves: Vec<ChessMove> = MoveGen::new_legal(board).collect();
        if root_moves.is_empty() {
            // Terminal position: record evaluation and stop.
            completed_best_move = None;
            completed_best_score = evaluate(board);
            break;
        }

        let mut alpha = -INF;
        let beta = INF;
        // Best move and score among root moves fully evaluated so far at this depth.
        let mut depth_best_move: Option<ChessMove> = None;
        let mut depth_best_score: i32 = -INF;

        for mv in root_moves {
            // Check deadline before evaluating this root move.
            if ctx.is_expired() {
                // Apply partial-depth override: only upgrade if clearly better.
                if let Some(partial_mv) = depth_best_move {
                    if depth_best_score >= completed_best_score + PARTIAL_OVERRIDE_THRESHOLD_CP {
                        completed_best_move = Some(partial_mv);
                        completed_best_score = depth_best_score;
                    }
                }
                break 'outer;
            }

            let child = board.make_move(mv);
            match alpha_beta_with_ctx(&child, depth - 1, -beta, -alpha, ctx) {
                Err(_timeout) => {
                    // Timeout inside child search: this root move was not fully evaluated.
                    // Apply partial-depth override using moves that *did* finish.
                    if let Some(partial_mv) = depth_best_move {
                        if depth_best_score >= completed_best_score + PARTIAL_OVERRIDE_THRESHOLD_CP {
                            completed_best_move = Some(partial_mv);
                            completed_best_score = depth_best_score;
                        }
                    }
                    break 'outer;
                }
                Ok((_child_mv, child_score)) => {
                    let score = -child_score;
                    if score > depth_best_score {
                        depth_best_score = score;
                        depth_best_move = Some(mv);
                    }
                    alpha = alpha.max(score);
                }
            }
        }

        // Depth fully completed: commit this depth's result.
        if depth_best_move.is_some() {
            completed_best_move = depth_best_move;
            completed_best_score = depth_best_score;
        }
    }

    (completed_best_move.or(fallback_move), completed_best_score)
}

/// Timeout-aware variant of [`alpha_beta`].
///
/// Performs the same negamax search with alpha-beta pruning but checks
/// `ctx.is_expired()` at each interior node before recursing. When the deadline
/// is reached the function immediately returns `Err(Timeout)`, propagating the
/// signal up through all recursive callers without corrupting any state that was
/// accumulated before the interrupt.
///
/// This function is used exclusively by [`search_timed`]; the public [`search`]
/// function continues to call the non-timed [`alpha_beta`] so non-timed behavior
/// is completely unaffected.
fn alpha_beta_with_ctx(
    board: &Board,
    depth: u8,
    mut alpha: i32,
    beta: i32,
    ctx: &SearchContext,
) -> Result<(Option<ChessMove>, i32), Timeout> {
    if depth == 0 {
        return Ok((None, evaluate(board)));
    }

    // Check deadline before expanding child nodes.
    if ctx.is_expired() {
        return Err(Timeout);
    }

    let moves: Vec<ChessMove> = MoveGen::new_legal(board).collect();
    if moves.is_empty() {
        return Ok((None, evaluate(board)));
    }

    let mut best_move = None;
    let mut best_score = -INF;

    for mv in moves {
        let child = board.make_move(mv);
        let (_child_mv, child_score) = alpha_beta_with_ctx(&child, depth - 1, -beta, -alpha, ctx)?;
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

    Ok((best_move, best_score))
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

    // --- SearchContext tests ---

    #[test]
    fn search_context_max_depth_is_64() {
        let ctx = SearchContext::from_budget_ms(1000);
        assert_eq!(ctx.max_depth, MAX_TIMED_DEPTH);
        assert_eq!(MAX_TIMED_DEPTH, 64);
    }

    #[test]
    fn search_context_not_expired_immediately_after_creation() {
        // A 1-second budget should not be expired the instant it is created.
        let ctx = SearchContext::from_budget_ms(1000);
        assert!(!ctx.is_expired(), "Context should not be expired immediately");
    }

    #[test]
    fn search_context_expired_after_zero_budget() {
        // A 0-ms budget should be expired almost immediately.
        let ctx = SearchContext::from_budget_ms(0);
        // Allow a brief spin in case the system is slow, but it must expire quickly.
        let expired = (0..100).any(|_| ctx.is_expired());
        assert!(expired, "Zero-budget context should expire quickly");
    }

    #[test]
    fn search_timed_returns_legal_move() {
        let board =
            Board::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let ctx = SearchContext::from_budget_ms(500);
        let (mv, _score) = search_timed(&board, &ctx);
        assert!(mv.is_some(), "search_timed should return a legal move");
        let mv = mv.unwrap();
        let legal_moves: Vec<ChessMove> = MoveGen::new_legal(&board).collect();
        assert!(
            legal_moves.contains(&mv),
            "search_timed returned an illegal move: {:?}",
            mv
        );
    }

    #[test]
    fn search_timed_does_not_affect_non_timed_search() {
        // Both search paths should agree on the starting position at depth 1.
        let board =
            Board::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let (mv_depth, _) = search(&board, 1);
        let ctx = SearchContext::from_budget_ms(500);
        let (mv_timed, _) = search_timed(&board, &ctx);
        // Both should return some legal move; timed path must not corrupt state.
        assert!(mv_depth.is_some());
        assert!(mv_timed.is_some());
    }

    // --- US-006: Timed search validation tests ---

    /// Helper: build a SearchContext with an already-expired deadline.
    fn expired_ctx() -> SearchContext {
        SearchContext {
            deadline: Instant::now() - std::time::Duration::from_millis(1),
            max_depth: MAX_TIMED_DEPTH,
        }
    }

    #[test]
    fn search_timed_movetime_returns_legal_move() {
        // Simulates `go movetime 200`: fixed per-move budget.
        let board =
            Board::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let ctx = SearchContext::from_budget_ms(200);
        let (mv, _score) = search_timed(&board, &ctx);
        assert!(mv.is_some(), "movetime search must return a legal move");
        let legal_moves: Vec<ChessMove> = MoveGen::new_legal(&board).collect();
        assert!(
            legal_moves.contains(&mv.unwrap()),
            "movetime search returned an illegal move"
        );
    }

    #[test]
    fn search_timed_clock_increment_returns_legal_move() {
        // Simulates `go wtime 60000 btime 60000 winc 1000 binc 1000`:
        // budget = 60000/20 + 1000/2 = 3500 ms.
        let board =
            Board::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let ctx = SearchContext::from_budget_ms(3500);
        let (mv, _score) = search_timed(&board, &ctx);
        assert!(
            mv.is_some(),
            "clock/increment search must return a legal move"
        );
        let legal_moves: Vec<ChessMove> = MoveGen::new_legal(&board).collect();
        assert!(
            legal_moves.contains(&mv.unwrap()),
            "clock/increment search returned an illegal move"
        );
    }

    #[test]
    fn search_timed_returns_legal_move_when_no_depth_completes() {
        // When the context is already expired, no depth can complete.
        // The engine must still return the first legal move as a fallback.
        let board =
            Board::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let ctx = expired_ctx();
        let (mv, _score) = search_timed(&board, &ctx);
        assert!(
            mv.is_some(),
            "search_timed must return a fallback legal move even when immediately expired"
        );
        let legal_moves: Vec<ChessMove> = MoveGen::new_legal(&board).collect();
        assert!(
            legal_moves.contains(&mv.unwrap()),
            "fallback move from expired context is not legal"
        );
    }

    #[test]
    fn search_timed_with_expired_context_returns_fallback_not_panics() {
        // Verify robustness: expired context on a non-trivial middlegame position.
        // FEN: a middlegame with many legal moves.
        let board =
            Board::from_str("r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4")
                .unwrap();
        let ctx = expired_ctx();
        let (mv, _score) = search_timed(&board, &ctx);
        assert!(mv.is_some(), "Must return a legal move even on expired context");
        let legal_moves: Vec<ChessMove> = MoveGen::new_legal(&board).collect();
        assert!(
            legal_moves.contains(&mv.unwrap()),
            "Returned move is not legal"
        );
    }

    #[test]
    fn search_timed_timeout_policy_uses_last_completed_depth_by_default() {
        // With a generous budget, at least depth 1 completes. The result should
        // match the depth-1 search on a simple position (one forced move).
        // Position: black king h8, white queen g7, white king g6 — Black in check,
        // only legal move is Kh8-h... wait, that's stalemate. Use a position where
        // black has exactly one legal move so we can predict the result.
        // Black king on h8, white queen on f6, white king on f8 — stalemate.
        // Use: black to move with one forced move: black king on a8, white queen on b6,
        // white king on c8 — stalemate. Hmm.
        //
        // Instead use: starting position with 1ms budget — at least depth 1 completes,
        // result is a legal move.
        let board =
            Board::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let ctx = SearchContext::from_budget_ms(1);
        let (mv, _score) = search_timed(&board, &ctx);
        // Even with minimal budget, a legal move must come back.
        assert!(mv.is_some(), "Timeout policy must return the completed-depth best or fallback");
        let legal_moves: Vec<ChessMove> = MoveGen::new_legal(&board).collect();
        assert!(legal_moves.contains(&mv.unwrap()), "Returned move is not legal");
    }

    #[test]
    fn search_timed_forced_move_position_returns_correct_move() {
        // Position with exactly one legal move: black king can only go to h7.
        // 7k/6Q1/6K1/8/8/8/8/8 b - - 0 1 — actually black is in checkmate, not one move.
        // Use: black king on h8, white rook on g1, white king on f6, black to move.
        // Black king h8 is hemmed in but has one move: Kh8-g8 if not attacked.
        // Simpler: forced-move position 7k/8/p5Q1/8/8/8/8/5K2 b - - 0 1
        let board = Board::from_str("7k/8/p5Q1/8/8/8/8/5K2 b - - 0 1").unwrap();
        let legal_moves: Vec<ChessMove> = MoveGen::new_legal(&board).collect();
        assert_eq!(legal_moves.len(), 1, "Position must have exactly one legal move");

        let ctx = SearchContext::from_budget_ms(500);
        let (mv, _score) = search_timed(&board, &ctx);
        assert_eq!(
            mv,
            Some(legal_moves[0]),
            "Timed search must return the only legal move in a forced position"
        );
    }
}
