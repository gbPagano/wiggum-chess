use async_trait::async_trait;
use chesslib::chess_move::ChessMove;
use chesslib::engine::{Engine, TimeControl};
use chesslib::game::Game;
use chesslib::movegen::MoveGen;

use crate::search::search;

/// In-process chess engine using material evaluation and negamax search.
pub struct MaterialEngine {
    depth: u8,
    last_game: Option<Game>,
}

impl MaterialEngine {
    /// Create a new MaterialEngine with the given search depth.
    pub fn new(depth: u8) -> Self {
        Self {
            depth,
            last_game: None,
        }
    }
}

impl Default for MaterialEngine {
    fn default() -> Self {
        Self::new(4)
    }
}

#[async_trait]
impl Engine for MaterialEngine {
    async fn name(&self) -> String {
        "MaterialEngine v0.1".to_string()
    }

    async fn new_game(&mut self) {
        self.last_game = None;
    }

    async fn set_position(&mut self, game: &Game) {
        self.last_game = Some(game.clone());
    }

    async fn go(&mut self, _time_control: &TimeControl) -> ChessMove {
        let game = self.last_game.as_ref().expect("set_position not called");
        let board = game.board();
        if let (Some(mv), _) = search(board, self.depth) {
            mv
        } else {
            // Fallback: return first legal move (should not happen in non-terminal positions)
            MoveGen::new_legal(board).next().expect("no legal moves")
        }
    }

    async fn quit(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use chesslib::game::GameResult;
    use chesslib::match_runner::Match;

    #[tokio::test]
    async fn material_engine_vs_self_completes_game() {
        let white: Box<dyn Engine> = Box::new(MaterialEngine::new(2));
        let black: Box<dyn Engine> = Box::new(MaterialEngine::new(2));
        let mut m = Match::new(white, black);
        let result = m.run().await;
        assert_ne!(
            result,
            GameResult::Ongoing,
            "Game must end with a definitive result"
        );
    }

    #[tokio::test]
    async fn material_engine_result_is_valid_variant() {
        let white: Box<dyn Engine> = Box::new(MaterialEngine::new(2));
        let black: Box<dyn Engine> = Box::new(MaterialEngine::new(2));
        let mut m = Match::new(white, black);
        let result = m.run().await;
        // Verify it's one of the valid terminal variants
        match result {
            GameResult::WhiteWins | GameResult::BlackWins | GameResult::Draw(_) => {}
            GameResult::Ongoing => panic!("Game must not be Ongoing after run()"),
        }
    }
}
