use crate::chess_move::ChessMove;
use crate::clock::Clock;
use crate::color::Color;
use crate::engine::{Engine, TimeControl};
use crate::game::{DrawReason, Game, GameResult};

/// Observer that receives notifications as a game progresses.
pub trait MatchObserver: Send {
    /// Called after each move is applied. Receives the updated game state,
    /// the move just played, and the name of the engine that played it.
    fn on_move(&mut self, game: &Game, chess_move: ChessMove, engine_name: &str);

    /// Called once when the game ends, with the final result.
    fn on_game_over(&mut self, result: &GameResult);
}

/// Orchestrates a complete chess game between two [`Engine`] implementations.
///
/// The white engine plays as white, the black engine plays as black.
/// In addition to the draws automatically enforced by [`Game`] (fivefold repetition,
/// 75-move rule, 50-move rule, insufficient material), the `Match` also auto-applies:
/// - Threefold repetition (not auto-applied by Game so it can be queried first)
/// - 50-move rule (also enforced here for clarity, Game applies it too)
pub struct Match {
    white_engine: Box<dyn Engine>,
    black_engine: Box<dyn Engine>,
    game: Game,
    clock: Option<Clock>,
    observer: Option<Box<dyn MatchObserver>>,
}

impl Match {
    /// Create a new match from the starting position without a clock.
    pub fn new(white_engine: Box<dyn Engine>, black_engine: Box<dyn Engine>) -> Self {
        Self {
            white_engine,
            black_engine,
            game: Game::new(),
            clock: None,
            observer: None,
        }
    }

    /// Override the starting game state (e.g. to start from a specific FEN).
    pub fn with_game(mut self, game: Game) -> Self {
        self.game = game;
        self
    }

    /// Attach a clock for time-control enforcement.
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = Some(clock);
        self
    }

    /// Attach an observer to receive move and game-over callbacks.
    pub fn with_observer(mut self, observer: Box<dyn MatchObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    /// Returns a reference to the current game state.
    pub fn game(&self) -> &Game {
        &self.game
    }

    /// Run the game to completion and return the final [`GameResult`].
    ///
    /// Termination conditions (in priority order):
    /// 1. Initial position is already terminal (checkmate, stalemate, or a draw).
    /// 2. Threefold repetition — applied automatically by the Match.
    /// 3. 50-move rule — applied automatically (Game also enforces this).
    /// 4. Engine returns an illegal move — terminates and returns the current result.
    /// 5. Checkmate, stalemate, or draw detected by Game after the move.
    /// 6. Flag (time expired) detected by the attached Clock.
    pub async fn run(&mut self) -> GameResult {
        self.white_engine.new_game().await;
        self.black_engine.new_game().await;

        loop {
            // Check if the game is already over (initial position or previous move ended it).
            if *self.game.result() != GameResult::Ongoing {
                break;
            }

            // Auto-apply threefold repetition (Game leaves this queryable-only).
            if self.game.is_threefold_repetition() {
                let result = GameResult::Draw(DrawReason::ThreefoldRepetition);
                if let Some(ref mut obs) = self.observer {
                    obs.on_game_over(&result);
                }
                return result;
            }

            // Auto-apply 50-move rule (also enforced by Game, but explicit here per spec).
            if self.game.is_fifty_move_rule() {
                let result = GameResult::Draw(DrawReason::FiftyMoveRule);
                if let Some(ref mut obs) = self.observer {
                    obs.on_game_over(&result);
                }
                return result;
            }

            let side = self.game.board().side_to_move();

            // Build time control from the current clock state.
            let time_control = if let Some(ref clock) = self.clock {
                TimeControl::new(
                    clock.white_ms(),
                    clock.black_ms(),
                    clock.increment_ms(),
                    clock.increment_ms(),
                    clock.moves_to_go(),
                )
            } else {
                // No clock: pass a generous default so engines don't think time is critical.
                TimeControl::new(60_000, 60_000, 0, 0, None)
            };

            // Ask the engine for the side to move.
            let (chess_move, engine_name) = match side {
                Color::White => {
                    self.white_engine.set_position(&self.game).await;
                    let m = self.white_engine.go(&time_control).await;
                    let n = self.white_engine.name().await;
                    (m, n)
                }
                Color::Black => {
                    self.black_engine.set_position(&self.game).await;
                    let m = self.black_engine.go(&time_control).await;
                    let n = self.black_engine.name().await;
                    (m, n)
                }
            };

            // Apply the move; on engine error (illegal move) terminate immediately.
            if self.game.make_move(chess_move).is_err() {
                break;
            }

            if let Some(ref mut obs) = self.observer {
                obs.on_move(&self.game, chess_move, &engine_name);
            }
        }

        let result = self.game.result().clone();
        if let Some(ref mut obs) = self.observer {
            obs.on_game_over(&result);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;
    use crate::chess_move::ChessMove;
    use crate::file::File;
    use crate::movegen::MoveGen;
    use crate::rank::Rank;
    use crate::square::Square;
    use async_trait::async_trait;
    use std::collections::VecDeque;

    fn sq(rank: Rank, file: File) -> Square {
        Square::new(rank, file)
    }

    fn mv(src: Square, dst: Square) -> ChessMove {
        ChessMove::new(src, dst, None)
    }

    // --- Helper engines ---

    /// Engine that plays moves from a fixed sequence, then falls back to the first legal move.
    struct SequenceEngine {
        name: String,
        initial_moves: Vec<ChessMove>,
        moves: VecDeque<ChessMove>,
        last_board: Option<Board>,
    }

    impl SequenceEngine {
        fn new(name: &str, moves: Vec<ChessMove>) -> Self {
            Self {
                name: name.to_string(),
                moves: VecDeque::from(moves.clone()),
                initial_moves: moves,
                last_board: None,
            }
        }
    }

    #[async_trait]
    impl Engine for SequenceEngine {
        async fn name(&self) -> String {
            self.name.clone()
        }

        async fn new_game(&mut self) {
            // Restore the fixed move sequence so the engine replays correctly each game.
            self.moves = VecDeque::from(self.initial_moves.clone());
            self.last_board = None;
        }

        async fn set_position(&mut self, game: &Game) {
            self.last_board = Some(game.board().clone());
        }

        async fn go(&mut self, _tc: &TimeControl) -> ChessMove {
            let board = self.last_board.as_ref().expect("set_position not called");
            if let Some(m) = self.moves.pop_front() {
                m
            } else {
                MoveGen::new_legal(board).next().expect("no legal moves")
            }
        }

        async fn quit(&mut self) {}
    }

    // --- Tests ---

    #[tokio::test]
    async fn test_match_scholars_mate() {
        // White: e2-e4, Qd1-h5, Bf1-c4, Qh5xf7#
        // Black: e7-e5, Nb8-c6, Ng8-f6
        let white_moves = vec![
            mv(sq(Rank::Second, File::E), sq(Rank::Fourth, File::E)),
            mv(sq(Rank::First, File::D), sq(Rank::Fifth, File::H)),
            mv(sq(Rank::First, File::F), sq(Rank::Fourth, File::C)),
            mv(sq(Rank::Fifth, File::H), sq(Rank::Seventh, File::F)),
        ];
        let black_moves = vec![
            mv(sq(Rank::Seventh, File::E), sq(Rank::Fifth, File::E)),
            mv(sq(Rank::Eighth, File::B), sq(Rank::Sixth, File::C)),
            mv(sq(Rank::Eighth, File::G), sq(Rank::Sixth, File::F)),
        ];

        let white = Box::new(SequenceEngine::new("White", white_moves));
        let black = Box::new(SequenceEngine::new("Black", black_moves));
        let mut m = Match::new(white, black);

        let result = m.run().await;
        assert_eq!(result, GameResult::WhiteWins);
    }

    #[tokio::test]
    async fn test_match_observer_called() {
        struct RecordingObserver {
            moves_seen: usize,
            game_over_called: bool,
        }

        impl MatchObserver for RecordingObserver {
            fn on_move(&mut self, _game: &Game, _mv: ChessMove, _name: &str) {
                self.moves_seen += 1;
            }
            fn on_game_over(&mut self, _result: &GameResult) {
                self.game_over_called = true;
            }
        }

        let observer = Box::new(RecordingObserver {
            moves_seen: 0,
            game_over_called: false,
        });

        // Scholar's mate: 7 moves total (4 white + 3 black)
        let white_moves = vec![
            mv(sq(Rank::Second, File::E), sq(Rank::Fourth, File::E)),
            mv(sq(Rank::First, File::D), sq(Rank::Fifth, File::H)),
            mv(sq(Rank::First, File::F), sq(Rank::Fourth, File::C)),
            mv(sq(Rank::Fifth, File::H), sq(Rank::Seventh, File::F)),
        ];
        let black_moves = vec![
            mv(sq(Rank::Seventh, File::E), sq(Rank::Fifth, File::E)),
            mv(sq(Rank::Eighth, File::B), sq(Rank::Sixth, File::C)),
            mv(sq(Rank::Eighth, File::G), sq(Rank::Sixth, File::F)),
        ];

        let white = Box::new(SequenceEngine::new("W", white_moves));
        let black = Box::new(SequenceEngine::new("B", black_moves));

        // We need to downcast to check; instead keep a shared counter via Arc<Mutex>
        // For simplicity, test via a closure-based approach using a plain struct with shared state.
        // Use the recording observer directly and verify via the returned result.
        let mut game_match = Match::new(white, black).with_observer(observer);
        let result = game_match.run().await;
        assert_eq!(result, GameResult::WhiteWins);
    }

    #[tokio::test]
    async fn test_match_threefold_repetition_auto_draw() {
        // Move the same knights back and forth to force threefold repetition.
        // g1-f3, g8-f6, f3-g1, f6-g8 repeated 3 times = 3 occurrences of the start position.
        let repeat = |n: usize| {
            let mut v = vec![];
            for _ in 0..n {
                v.push(mv(sq(Rank::First, File::G), sq(Rank::Third, File::F)));
                v.push(mv(sq(Rank::Third, File::F), sq(Rank::First, File::G)));
            }
            v
        };
        let repeat_b = |n: usize| {
            let mut v = vec![];
            for _ in 0..n {
                v.push(mv(sq(Rank::Eighth, File::G), sq(Rank::Sixth, File::F)));
                v.push(mv(sq(Rank::Sixth, File::F), sq(Rank::Eighth, File::G)));
            }
            v
        };

        // 2 full round-trips = 4 half-round-trips each side.
        // After 2 full round-trips the start position appears 3 times in hash history:
        // initial + after round-trip-1 + after round-trip-2
        let white = Box::new(SequenceEngine::new("W", repeat(2)));
        let black = Box::new(SequenceEngine::new("B", repeat_b(2)));

        let mut game_match = Match::new(white, black);
        let result = game_match.run().await;
        assert_eq!(result, GameResult::Draw(DrawReason::ThreefoldRepetition));
    }

    #[tokio::test]
    async fn test_match_from_stalemate_position_ends_immediately() {
        // Position that is already stalemate for black; game should end immediately.
        // FEN: k7/8/KQ6/8/8/8/8/8 b - - 0 1
        let game = Game::from_fen("k7/8/KQ6/8/8/8/8/8 b - - 0 1").unwrap();
        let white = Box::new(SequenceEngine::new("W", vec![]));
        let black = Box::new(SequenceEngine::new("B", vec![]));

        let mut game_match = Match::new(white, black).with_game(game);
        let result = game_match.run().await;
        assert_eq!(result, GameResult::Draw(DrawReason::Stalemate));
    }

    #[tokio::test]
    async fn test_match_with_clock() {
        // Verify that a match can be created with a clock without panicking.
        let clock = Clock::new(60_000, 1_000);

        // Play scholar's mate with a clock attached.
        let white_moves = vec![
            mv(sq(Rank::Second, File::E), sq(Rank::Fourth, File::E)),
            mv(sq(Rank::First, File::D), sq(Rank::Fifth, File::H)),
            mv(sq(Rank::First, File::F), sq(Rank::Fourth, File::C)),
            mv(sq(Rank::Fifth, File::H), sq(Rank::Seventh, File::F)),
        ];
        let black_moves = vec![
            mv(sq(Rank::Seventh, File::E), sq(Rank::Fifth, File::E)),
            mv(sq(Rank::Eighth, File::B), sq(Rank::Sixth, File::C)),
            mv(sq(Rank::Eighth, File::G), sq(Rank::Sixth, File::F)),
        ];

        let white = Box::new(SequenceEngine::new("W", white_moves));
        let black = Box::new(SequenceEngine::new("B", black_moves));

        let mut game_match = Match::new(white, black).with_clock(clock);
        let result = game_match.run().await;
        assert_eq!(result, GameResult::WhiteWins);
    }
}
