use async_trait::async_trait;

use crate::chess_move::ChessMove;
use crate::game::Game;

/// Time control parameters passed to the engine for thinking.
///
/// All time values are in milliseconds.
#[derive(Clone, Debug)]
pub struct TimeControl {
    /// White's remaining time in milliseconds.
    pub wtime: u64,
    /// Black's remaining time in milliseconds.
    pub btime: u64,
    /// White's increment per move in milliseconds.
    pub winc: u64,
    /// Black's increment per move in milliseconds.
    pub binc: u64,
    /// Number of moves until the next time control boundary (classical time controls).
    pub movestogo: Option<u32>,
}

impl TimeControl {
    /// Create a new time control with the given parameters.
    pub fn new(wtime: u64, btime: u64, winc: u64, binc: u64, movestogo: Option<u32>) -> Self {
        Self {
            wtime,
            btime,
            winc,
            binc,
            movestogo,
        }
    }
}

/// Abstraction over a chess engine that can receive positions and return moves.
///
/// Implementations may be in-process engines or UCI subprocess engines.
/// The trait uses `async_trait` to support both `Box<dyn Engine>` usage and
/// async implementations backed by tokio.
#[async_trait]
pub trait Engine: Send {
    /// Returns the engine's name.
    async fn name(&self) -> String;

    /// Notifies the engine to start a new game, clearing internal state.
    async fn new_game(&mut self);

    /// Sets the current position from the given game's history.
    async fn set_position(&mut self, game: &Game);

    /// Asks the engine to search and return the best move given the time control.
    async fn go(&mut self, time_control: &TimeControl) -> ChessMove;

    /// Shuts down the engine.
    async fn quit(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;
    use crate::chess_move::ChessMove;
    use crate::movegen::MoveGen;

    struct FirstMoveEngine;

    #[async_trait]
    impl Engine for FirstMoveEngine {
        async fn name(&self) -> String {
            "FirstMoveEngine".to_string()
        }

        async fn new_game(&mut self) {}

        async fn set_position(&mut self, _game: &Game) {}

        async fn go(&mut self, _time_control: &TimeControl) -> ChessMove {
            let board = Board::default();
            MoveGen::new_legal(&board).next().unwrap()
        }

        async fn quit(&mut self) {}
    }

    #[tokio::test]
    async fn test_engine_trait_can_be_implemented() {
        let mut engine = FirstMoveEngine;
        assert_eq!(engine.name().await, "FirstMoveEngine");
        engine.new_game().await;
        let game = Game::new();
        engine.set_position(&game).await;
        let tc = TimeControl::new(5000, 5000, 100, 100, None);
        let _mv = engine.go(&tc).await;
        engine.quit().await;
    }

    #[tokio::test]
    async fn test_engine_trait_object_safe() {
        let mut engine: Box<dyn Engine> = Box::new(FirstMoveEngine);
        assert_eq!(engine.name().await, "FirstMoveEngine");
        engine.new_game().await;
        engine.quit().await;
    }

    #[test]
    fn test_time_control_fields() {
        let tc = TimeControl::new(60000, 55000, 1000, 1000, Some(40));
        assert_eq!(tc.wtime, 60000);
        assert_eq!(tc.btime, 55000);
        assert_eq!(tc.winc, 1000);
        assert_eq!(tc.binc, 1000);
        assert_eq!(tc.movestogo, Some(40));
    }

    #[test]
    fn test_time_control_no_movestogo() {
        let tc = TimeControl::new(30000, 30000, 0, 0, None);
        assert_eq!(tc.movestogo, None);
    }
}
