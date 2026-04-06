use std::time::Instant;

use crate::color::Color;

/// A chess clock tracking remaining time per player with optional increment.
///
/// Supports both sudden-death (total time + increment) and classical (moves-to-go)
/// time control configurations.
#[derive(Clone)]
pub struct Clock {
    white_ms: u64,
    black_ms: u64,
    increment_ms: u64,
    /// Remaining moves until the next time control, if using classical time controls.
    moves_to_go: Option<u32>,
    turn_start: Instant,
}

impl Clock {
    /// Create a new clock with equal time for both players and an increment.
    pub fn new(time_ms: u64, increment_ms: u64) -> Self {
        Self {
            white_ms: time_ms,
            black_ms: time_ms,
            increment_ms,
            moves_to_go: None,
            turn_start: Instant::now(),
        }
    }

    /// Create a new clock with classical time control (moves-to-go).
    pub fn with_moves_to_go(time_ms: u64, increment_ms: u64, moves_to_go: u32) -> Self {
        Self {
            white_ms: time_ms,
            black_ms: time_ms,
            increment_ms,
            moves_to_go: Some(moves_to_go),
            turn_start: Instant::now(),
        }
    }

    pub fn white_ms(&self) -> u64 {
        self.white_ms
    }

    pub fn black_ms(&self) -> u64 {
        self.black_ms
    }

    pub fn increment_ms(&self) -> u64 {
        self.increment_ms
    }

    pub fn moves_to_go(&self) -> Option<u32> {
        self.moves_to_go
    }

    /// Returns the remaining time in milliseconds for the given color.
    pub fn remaining_ms(&self, color: Color) -> u64 {
        match color {
            Color::White => self.white_ms,
            Color::Black => self.black_ms,
        }
    }

    /// Returns true if the given player has no time remaining (flag fell).
    pub fn is_flagged(&self, color: Color) -> bool {
        self.remaining_ms(color) == 0
    }

    /// Reset the turn timer. Call at the start of a player's turn.
    pub fn start_turn(&mut self) {
        self.turn_start = Instant::now();
    }

    /// Record that the given player made a move: subtracts elapsed time and adds increment.
    /// Returns `true` if the player's flag falls (time expired).
    pub fn record_move(&mut self, color: Color) -> bool {
        let elapsed = self.turn_start.elapsed().as_millis() as u64;
        self.record_move_with_elapsed(color, elapsed)
    }

    /// Like `record_move` but with an explicit elapsed-ms value — useful for testing.
    pub(crate) fn record_move_with_elapsed(&mut self, color: Color, elapsed_ms: u64) -> bool {
        self.turn_start = Instant::now();

        let remaining = match color {
            Color::White => &mut self.white_ms,
            Color::Black => &mut self.black_ms,
        };

        if elapsed_ms >= *remaining {
            *remaining = 0;
            return true; // flag
        }

        *remaining -= elapsed_ms;
        *remaining = remaining.saturating_add(self.increment_ms);

        if let Some(ref mut mtg) = self.moves_to_go {
            *mtg = mtg.saturating_sub(1);
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_new_sets_time() {
        let clock = Clock::new(60_000, 1_000);
        assert_eq!(clock.white_ms(), 60_000);
        assert_eq!(clock.black_ms(), 60_000);
        assert_eq!(clock.increment_ms(), 1_000);
        assert_eq!(clock.moves_to_go(), None);
    }

    #[test]
    fn test_clock_with_moves_to_go() {
        let clock = Clock::with_moves_to_go(40_000, 0, 40);
        assert_eq!(clock.white_ms(), 40_000);
        assert_eq!(clock.moves_to_go(), Some(40));
    }

    #[test]
    fn test_clock_record_move_decrements_and_adds_increment() {
        let mut clock = Clock::new(60_000, 1_000);
        // White uses 5 seconds
        let flagged = clock.record_move_with_elapsed(Color::White, 5_000);
        assert!(!flagged);
        // 60000 - 5000 + 1000 = 56000
        assert_eq!(clock.white_ms(), 56_000);
        // Black time unchanged
        assert_eq!(clock.black_ms(), 60_000);
    }

    #[test]
    fn test_clock_flag_when_time_runs_out() {
        let mut clock = Clock::new(5_000, 0);
        // White uses more than their remaining time
        let flagged = clock.record_move_with_elapsed(Color::White, 6_000);
        assert!(flagged);
        assert_eq!(clock.white_ms(), 0);
        assert!(clock.is_flagged(Color::White));
        assert!(!clock.is_flagged(Color::Black));
    }

    #[test]
    fn test_clock_flag_exact_exhaustion() {
        let mut clock = Clock::new(1_000, 0);
        // White uses exactly their remaining time
        let flagged = clock.record_move_with_elapsed(Color::White, 1_000);
        assert!(flagged);
        assert!(clock.is_flagged(Color::White));
    }

    #[test]
    fn test_clock_moves_to_go_decrements() {
        let mut clock = Clock::with_moves_to_go(60_000, 0, 40);
        clock.record_move_with_elapsed(Color::White, 1_000);
        assert_eq!(clock.moves_to_go(), Some(39));
    }

    #[test]
    fn test_clock_no_flag_zero_increment_enough_time() {
        let mut clock = Clock::new(10_000, 0);
        let flagged = clock.record_move_with_elapsed(Color::Black, 3_000);
        assert!(!flagged);
        assert_eq!(clock.black_ms(), 7_000);
    }

    #[test]
    fn test_clock_start_turn_resets_timer() {
        let mut clock = Clock::new(60_000, 0);
        // start_turn doesn't panic and resets the timer
        clock.start_turn();
        // Immediately record the move — should use ~0ms
        let flagged = clock.record_move(Color::White);
        assert!(!flagged);
        assert!(clock.white_ms() > 59_000); // well within tolerance
    }
}
