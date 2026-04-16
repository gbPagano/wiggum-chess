use chesslib::color::Color;

/// Parsed parameters from a UCI `go` command.
///
/// Captures timed search inputs: `movetime`, clock values (`wtime`/`btime`),
/// and increments (`winc`/`binc`). When none of these are present the engine
/// falls back to its default depth-based search.
#[derive(Debug, Default, PartialEq)]
pub struct GoParams {
    /// Fixed time allocated for this move in milliseconds (`go movetime <ms>`).
    pub movetime: Option<u64>,
    /// White's remaining clock time in milliseconds.
    pub wtime: Option<u64>,
    /// Black's remaining clock time in milliseconds.
    pub btime: Option<u64>,
    /// White's increment per move in milliseconds.
    pub winc: Option<u64>,
    /// Black's increment per move in milliseconds.
    pub binc: Option<u64>,
}

impl GoParams {
    /// Returns the remaining clock time for `color` in milliseconds, if present.
    pub fn remaining_time(&self, color: Color) -> Option<u64> {
        match color {
            Color::White => self.wtime,
            Color::Black => self.btime,
        }
    }

    /// Returns the increment for `color` in milliseconds, defaulting to 0.
    pub fn increment(&self, color: Color) -> u64 {
        match color {
            Color::White => self.winc.unwrap_or(0),
            Color::Black => self.binc.unwrap_or(0),
        }
    }

    /// Returns `true` if any timed parameter was provided.
    pub fn has_time_control(&self) -> bool {
        self.movetime.is_some()
            || self.wtime.is_some()
            || self.btime.is_some()
    }

    /// Compute the search budget in milliseconds for `color`.
    ///
    /// # Budgeting heuristic
    ///
    /// - `go movetime <ms>`: use the fixed value directly.
    /// - Clock-based (`wtime`/`btime`): allocate `remaining / 20 + increment / 2`
    ///   so the engine nominally has 20 moves left while also spending half the
    ///   increment per move.  The result is then capped at
    ///   `remaining - SAFETY_MARGIN_MS` to avoid flagging on slow systems.
    /// - Returns `None` when no timed parameter is present (fall back to
    ///   depth-based search).
    pub fn compute_budget_ms(&self, color: Color) -> Option<u64> {
        /// Minimum cushion kept in reserve to avoid overshooting the clock.
        const SAFETY_MARGIN_MS: u64 = 50;

        if let Some(movetime) = self.movetime {
            return Some(movetime);
        }

        let remaining = self.remaining_time(color)?;
        let increment = self.increment(color);

        let budget = remaining / 20 + increment / 2;

        // Cap the budget so we never spend more than remaining - safety_margin.
        let cap = remaining.saturating_sub(SAFETY_MARGIN_MS);
        Some(budget.min(cap))
    }
}

/// Parse a UCI `go` command line into a [`GoParams`].
///
/// Recognised tokens: `movetime`, `wtime`, `btime`, `winc`, `binc`.
/// All other tokens (e.g. `infinite`, `depth`, `nodes`) are silently ignored
/// to preserve forward compatibility with the UCI specification.
pub fn parse_go(line: &str) -> GoParams {
    let mut params = GoParams::default();
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let mut i = 1; // skip the leading "go" token

    while i < tokens.len() {
        let value = tokens.get(i + 1).and_then(|v| v.parse::<u64>().ok());
        match tokens[i] {
            "movetime" => {
                params.movetime = value;
                i += 2;
            }
            "wtime" => {
                params.wtime = value;
                i += 2;
            }
            "btime" => {
                params.btime = value;
                i += 2;
            }
            "winc" => {
                params.winc = value;
                i += 2;
            }
            "binc" => {
                params.binc = value;
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }

    params
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_go_movetime() {
        let p = parse_go("go movetime 500");
        assert_eq!(p.movetime, Some(500));
        assert!(p.wtime.is_none());
        assert!(p.has_time_control());
    }

    #[test]
    fn parse_go_clock_and_increment() {
        let p = parse_go("go wtime 60000 btime 60000 winc 1000 binc 1000");
        assert_eq!(p.wtime, Some(60000));
        assert_eq!(p.btime, Some(60000));
        assert_eq!(p.winc, Some(1000));
        assert_eq!(p.binc, Some(1000));
        assert!(p.movetime.is_none());
        assert!(p.has_time_control());
    }

    #[test]
    fn parse_go_selects_active_side_clock() {
        let p = parse_go("go wtime 10000 btime 20000 winc 500 binc 250");
        assert_eq!(p.remaining_time(Color::White), Some(10000));
        assert_eq!(p.remaining_time(Color::Black), Some(20000));
        assert_eq!(p.increment(Color::White), 500);
        assert_eq!(p.increment(Color::Black), 250);
    }

    #[test]
    fn parse_go_no_time_params_preserves_non_timed_behavior() {
        let p = parse_go("go depth 5");
        assert!(!p.has_time_control());
        assert!(p.movetime.is_none());
        assert!(p.wtime.is_none());
    }

    #[test]
    fn parse_go_ignores_unknown_tokens() {
        let p = parse_go("go infinite");
        assert!(!p.has_time_control());
    }

    #[test]
    fn parse_go_partial_clock_without_increment_defaults_increment_to_zero() {
        let p = parse_go("go wtime 5000 btime 5000");
        assert_eq!(p.increment(Color::White), 0);
        assert_eq!(p.increment(Color::Black), 0);
    }

    // --- compute_budget_ms tests ---

    #[test]
    fn budget_movetime_returned_directly() {
        let p = parse_go("go movetime 500");
        assert_eq!(p.compute_budget_ms(Color::White), Some(500));
        assert_eq!(p.compute_budget_ms(Color::Black), Some(500));
    }

    #[test]
    fn budget_clock_based_uses_remaining_over_20_plus_half_increment() {
        // remaining=60000, increment=1000 → 60000/20 + 1000/2 = 3000+500 = 3500
        let p = parse_go("go wtime 60000 btime 60000 winc 1000 binc 1000");
        assert_eq!(p.compute_budget_ms(Color::White), Some(3500));
        assert_eq!(p.compute_budget_ms(Color::Black), Some(3500));
    }

    #[test]
    fn budget_selects_active_side_clock() {
        // White: 20000ms remaining, 0 inc → 20000/20 = 1000
        // Black: 40000ms remaining, 2000 inc → 40000/20 + 1000 = 3000
        let p = parse_go("go wtime 20000 btime 40000 binc 2000");
        assert_eq!(p.compute_budget_ms(Color::White), Some(1000));
        assert_eq!(p.compute_budget_ms(Color::Black), Some(3000));
    }

    #[test]
    fn budget_capped_below_remaining_minus_safety_margin() {
        // remaining=100, increment=0 → budget=100/20=5; cap=100-50=50 → min(5,50)=5
        // Ensures cap only bites when budget exceeds remaining-50.
        // Now use tiny remaining where budget > cap:
        // remaining=60, increment=0 → budget=60/20=3; cap=60-50=10 → min(3,10)=3
        let p = GoParams { wtime: Some(60), ..Default::default() };
        assert_eq!(p.compute_budget_ms(Color::White), Some(3));

        // remaining=40, increment=0 → budget=40/20=2; cap=40-50=0 (saturating) → min(2,0)=0
        let p2 = GoParams { wtime: Some(40), ..Default::default() };
        assert_eq!(p2.compute_budget_ms(Color::White), Some(0));
    }

    #[test]
    fn budget_returns_none_when_no_time_control() {
        let p = parse_go("go depth 5");
        assert_eq!(p.compute_budget_ms(Color::White), None);
        assert_eq!(p.compute_budget_ms(Color::Black), None);
    }
}
