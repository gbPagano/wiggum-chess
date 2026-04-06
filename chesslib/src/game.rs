use crate::board::Board;
use crate::chess_move::ChessMove;
use crate::movegen::MoveGen;
use std::str::FromStr;

/// Reasons a game ended in a draw.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DrawReason {
    Stalemate,
    InsufficientMaterial,
    ThreefoldRepetition,
    FivefoldRepetition,
    FiftyMoveRule,
    SeventyFiveMoveRule,
}

/// Result of a chess game.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameResult {
    Ongoing,
    WhiteWins,
    BlackWins,
    Draw(DrawReason),
}

/// A complete chess game with move history, position hash history, and result tracking.
#[derive(Clone)]
pub struct Game {
    board: Board,
    moves: Vec<ChessMove>,
    hash_history: Vec<u64>,
    result: GameResult,
}

impl Game {
    /// Create a new game from the starting position.
    pub fn new() -> Self {
        let board = Board::default();
        let hash = board.zobrist_hash();
        let mut game = Self {
            board,
            moves: Vec::new(),
            hash_history: vec![hash],
            result: GameResult::Ongoing,
        };
        game.result = game.detect_result();
        game
    }

    /// Create a new game from a FEN string.
    pub fn from_fen(fen: &str) -> Result<Self, anyhow::Error> {
        let board = Board::from_str(fen)?;
        let hash = board.zobrist_hash();
        let mut game = Self {
            board,
            moves: Vec::new(),
            hash_history: vec![hash],
            result: GameResult::Ongoing,
        };
        game.result = game.detect_result();
        Ok(game)
    }

    /// Returns the current board state.
    pub fn board(&self) -> &Board {
        &self.board
    }

    /// Returns the move history.
    pub fn moves(&self) -> &[ChessMove] {
        &self.moves
    }

    /// Returns the current game result.
    pub fn result(&self) -> &GameResult {
        &self.result
    }

    /// Returns whether the threefold repetition condition is met (can be claimed as draw).
    pub fn is_threefold_repetition(&self) -> bool {
        let current = self.board.zobrist_hash();
        self.hash_history.iter().filter(|&&h| h == current).count() >= 3
    }

    /// Returns whether the fifty-move rule condition is met (can be claimed as draw).
    pub fn is_fifty_move_rule(&self) -> bool {
        self.board.halfmove_clock() >= 100
    }

    /// Apply a move to the game. Returns an error if the move is illegal or the game is already over.
    pub fn make_move(&mut self, m: ChessMove) -> Result<(), anyhow::Error> {
        if self.result != GameResult::Ongoing {
            anyhow::bail!("game is already over");
        }

        // Validate legality
        let legal = MoveGen::new_legal(&self.board)
            .find(|lm| *lm == m)
            .is_some();
        if !legal {
            anyhow::bail!("illegal move");
        }

        self.board = self.board.make_move(m);
        self.moves.push(m);
        let hash = self.board.zobrist_hash();
        self.hash_history.push(hash);

        self.result = self.detect_result();
        Ok(())
    }

    fn detect_result(&self) -> GameResult {
        // Fivefold repetition — applied automatically
        let current = self.board.zobrist_hash();
        let repetitions = self.hash_history.iter().filter(|&&h| h == current).count();
        if repetitions >= 5 {
            return GameResult::Draw(DrawReason::FivefoldRepetition);
        }

        // 75-move rule — applied automatically
        if self.board.halfmove_clock() >= 150 {
            return GameResult::Draw(DrawReason::SeventyFiveMoveRule);
        }

        // 50-move rule — applied automatically
        if self.board.halfmove_clock() >= 100 {
            return GameResult::Draw(DrawReason::FiftyMoveRule);
        }

        // Insufficient material
        if self.board.is_insufficient_material() {
            return GameResult::Draw(DrawReason::InsufficientMaterial);
        }

        // Checkmate / stalemate
        if self.board.is_checkmate() {
            // The side that just moved wins
            return match self.board.side_to_move() {
                crate::color::Color::White => GameResult::BlackWins,
                crate::color::Color::Black => GameResult::WhiteWins,
            };
        }

        if self.board.is_stalemate() {
            return GameResult::Draw(DrawReason::Stalemate);
        }

        GameResult::Ongoing
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::square::Square;
    use crate::rank::Rank;
    use crate::file::File;

    fn sq(rank: Rank, file: File) -> Square {
        Square::new(rank, file)
    }

    fn mv(src: Square, dst: Square) -> ChessMove {
        ChessMove::new(src, dst, None)
    }

    #[test]
    fn test_game_new_is_ongoing() {
        let game = Game::new();
        assert_eq!(*game.result(), GameResult::Ongoing);
        assert_eq!(game.moves().len(), 0);
    }

    #[test]
    fn test_game_illegal_move_rejected() {
        let mut game = Game::new();
        // e2 to e5 — not legal for a pawn
        let m = mv(sq(Rank::Second, File::E), sq(Rank::Fifth, File::E));
        assert!(game.make_move(m).is_err());
        assert_eq!(*game.result(), GameResult::Ongoing);
    }

    #[test]
    fn test_game_scholars_mate() {
        // 1. e4 e5 2. Qh5 Nc6 3. Bc4 Nf6? 4. Qxf7#
        let mut game = Game::new();
        game.make_move(mv(sq(Rank::Second, File::E), sq(Rank::Fourth, File::E))).unwrap();
        game.make_move(mv(sq(Rank::Seventh, File::E), sq(Rank::Fifth, File::E))).unwrap();
        game.make_move(mv(sq(Rank::First, File::D), sq(Rank::Fifth, File::H))).unwrap();
        game.make_move(mv(sq(Rank::Eighth, File::B), sq(Rank::Sixth, File::C))).unwrap();
        game.make_move(mv(sq(Rank::First, File::F), sq(Rank::Fourth, File::C))).unwrap();
        game.make_move(mv(sq(Rank::Eighth, File::G), sq(Rank::Sixth, File::F))).unwrap();
        game.make_move(mv(sq(Rank::Fifth, File::H), sq(Rank::Seventh, File::F))).unwrap();
        assert_eq!(*game.result(), GameResult::WhiteWins);
    }

    #[test]
    fn test_game_stalemate() {
        // A known stalemate position: White king a6, White queen b6, Black king a8
        // FEN: k7/8/KQ6/8/8/8/8/8 b - - 0 1
        // It's black's turn and black is stalemated
        let game = Game::from_fen("k7/8/KQ6/8/8/8/8/8 b - - 0 1").unwrap();
        assert_eq!(*game.result(), GameResult::Draw(DrawReason::Stalemate));
    }

    #[test]
    fn test_game_stalemate_after_moves() {
        // Position where stalemate occurs after one move
        // FEN: 8/8/8/8/8/1q6/8/K7 b - - 0 1  (not a stalemate yet)
        // Instead use a position where white is about to get stalemated:
        // "7k/5Q2/6K1/8/8/8/8/8 b - - 0 1" — stalemate for black
        let game = Game::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
        assert_eq!(*game.result(), GameResult::Draw(DrawReason::Stalemate));
    }

    #[test]
    fn test_game_insufficient_material_kk() {
        let game = Game::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        assert_eq!(*game.result(), GameResult::Draw(DrawReason::InsufficientMaterial));
    }

    #[test]
    fn test_game_threefold_repetition() {
        // Repeat the starting position 3 times by moving knights back and forth.
        // Threefold repetition is queryable (not auto-applied); game remains Ongoing.
        let mut game = Game::new();
        // g1-f3, g8-f6, f3-g1, f6-g8 — back to start (hash = start)
        // Do this twice: starting position will appear 3 times in hash_history
        for _ in 0..2 {
            game.make_move(mv(sq(Rank::First, File::G), sq(Rank::Third, File::F))).unwrap();
            game.make_move(mv(sq(Rank::Eighth, File::G), sq(Rank::Sixth, File::F))).unwrap();
            game.make_move(mv(sq(Rank::Third, File::F), sq(Rank::First, File::G))).unwrap();
            game.make_move(mv(sq(Rank::Sixth, File::F), sq(Rank::Eighth, File::G))).unwrap();
        }
        assert!(game.is_threefold_repetition());
        assert_eq!(*game.result(), GameResult::Ongoing);
    }

    #[test]
    fn test_game_fivefold_repetition() {
        let mut game = Game::new();
        for _ in 0..4 {
            game.make_move(mv(sq(Rank::First, File::G), sq(Rank::Third, File::F))).unwrap();
            game.make_move(mv(sq(Rank::Eighth, File::G), sq(Rank::Sixth, File::F))).unwrap();
            game.make_move(mv(sq(Rank::Third, File::F), sq(Rank::First, File::G))).unwrap();
            game.make_move(mv(sq(Rank::Sixth, File::F), sq(Rank::Eighth, File::G))).unwrap();
        }
        assert_eq!(*game.result(), GameResult::Draw(DrawReason::FivefoldRepetition));
    }

    #[test]
    fn test_game_fifty_move_rule() {
        // Use a position with two kings and two rooks — no captures or pawn moves possible
        // K+R vs K+R — make 100 halfmoves (50 full moves) without captures/pawns
        // FEN: 4k3/8/8/8/8/8/8/R3K2R w - - 99 1
        let game = Game::from_fen("4k3/8/8/8/8/8/8/R3K2R w - - 99 1").unwrap();
        // One more move to trigger
        // Actually the 50-move rule triggers at halfmove_clock >= 100 (after making the 100th half-move)
        // Here we start with clock=99; after one non-pawn/non-capture move -> clock=100 -> draw
        let mut game = game;
        // Move rook a1 to a2 (non-capture, non-pawn)
        game.make_move(mv(sq(Rank::First, File::A), sq(Rank::Second, File::A))).unwrap();
        assert_eq!(*game.result(), GameResult::Draw(DrawReason::FiftyMoveRule));
    }

    #[test]
    fn test_game_seventy_five_move_rule() {
        // FEN with halfmove_clock=150; 75-move rule triggers immediately on detection.
        // (75-move check comes before 50-move check in detect_result)
        let game = Game::from_fen("4k3/8/8/8/8/8/8/R3K2R w - - 150 1").unwrap();
        assert_eq!(*game.result(), GameResult::Draw(DrawReason::SeventyFiveMoveRule));
    }

    #[test]
    fn test_game_make_move_after_game_over_fails() {
        let mut game = Game::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        // Game is already a draw
        assert_eq!(*game.result(), GameResult::Draw(DrawReason::InsufficientMaterial));
        let m = mv(sq(Rank::First, File::E), sq(Rank::Second, File::E));
        assert!(game.make_move(m).is_err());
    }

    #[test]
    fn test_game_move_history() {
        let mut game = Game::new();
        let m1 = mv(sq(Rank::Second, File::E), sq(Rank::Fourth, File::E));
        let m2 = mv(sq(Rank::Seventh, File::E), sq(Rank::Fifth, File::E));
        game.make_move(m1).unwrap();
        game.make_move(m2).unwrap();
        assert_eq!(game.moves(), &[m1, m2]);
    }
}
