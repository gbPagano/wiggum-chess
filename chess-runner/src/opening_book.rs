/// Opening-book loader for chess-runner neutral opening support.
///
/// # Format
///
/// Opening books are plain-text files where each non-empty, non-comment line represents
/// one opening line as a sequence of UCI moves separated by spaces, starting from the
/// initial position.
///
/// ```text
/// # Comment lines (starting with '#') and empty lines are ignored.
/// e2e4 e7e5 g1f3 b8c6 f1b5
/// d2d4 d7d5 c2c4
/// ```
///
/// Each move token must be a valid UCI move string (e.g. `e2e4`, `e7e8q`).
/// The book can be loaded from a file bundled in the repository (e.g. `data/openings.txt`)
/// or from any local path supplied at runtime.
use chesslib::board::Board;
use chesslib::chess_move::ChessMove;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use std::path::Path;

/// A single opening line: a sequence of UCI move strings from the initial position.
#[derive(Debug, Clone)]
pub struct OpeningLine {
    /// Raw UCI move tokens (e.g. ["e2e4", "e7e5"]).
    pub moves: Vec<String>,
}

/// Load opening lines from a plain-text file.
///
/// Lines beginning with `#` and empty lines are silently skipped.
/// Each non-empty, non-comment line is split on whitespace to produce one [`OpeningLine`].
///
/// Returns an error if the file cannot be read.
pub fn load_opening_book(path: &Path) -> anyhow::Result<Vec<OpeningLine>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("cannot read opening book '{}': {}", path.display(), e))?;
    Ok(parse_opening_book(&content))
}

/// Parse opening lines from a string in the opening-book format.
///
/// This is the pure parsing step (no I/O), useful for testing.
pub fn parse_opening_book(content: &str) -> Vec<OpeningLine> {
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| OpeningLine {
            moves: l.split_whitespace().map(|s| s.to_string()).collect(),
        })
        .collect()
}

/// Validate a slice of parsed opening lines against legal chess moves.
///
/// Each line is replayed from the initial position. If any move in a line is illegal,
/// that line is rejected and an error message is returned for it. Lines that pass are
/// collected and returned. If the result is empty (all lines were invalid, or the input
/// was empty to begin with), an error is returned.
///
/// # Errors
///
/// Returns an error if any line contains an illegal move, or if no valid lines remain.
pub fn validate_opening_book(lines: Vec<OpeningLine>) -> anyhow::Result<Vec<OpeningLine>> {
    let mut valid = Vec::with_capacity(lines.len());
    let mut errors: Vec<String> = Vec::new();

    for line in lines {
        let mut board = Board::default();
        let mut ok = true;
        for (i, mv_str) in line.moves.iter().enumerate() {
            match ChessMove::from_uci(mv_str, &board) {
                Ok(m) => {
                    board = board.make_move(m);
                }
                Err(e) => {
                    errors.push(format!(
                        "invalid move '{}' at ply {} in line '{}': {}",
                        mv_str,
                        i + 1,
                        line.moves.join(" "),
                        e
                    ));
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            valid.push(line);
        }
    }

    if !errors.is_empty() {
        let msg = errors.join("\n");
        anyhow::bail!("opening book contains invalid lines:\n{}", msg);
    }

    if valid.is_empty() {
        anyhow::bail!("opening book is empty: no valid opening lines found");
    }

    Ok(valid)
}

/// Select one opening line uniformly at random from a non-empty validated set.
///
/// If `seed` is `Some(s)`, the same seed and same input always select the same line.
/// If `seed` is `None`, a random non-deterministic seed is used.
///
/// # Panics
///
/// Panics if `lines` is empty (callers must ensure the book is non-empty, e.g. via
/// [`validate_opening_book`]).
pub fn select_opening_line(lines: &[OpeningLine], seed: Option<u64>) -> &OpeningLine {
    assert!(!lines.is_empty(), "select_opening_line called with empty book");
    let effective_seed = seed.unwrap_or_else(rand::random::<u64>);
    let mut rng = rand::rngs::SmallRng::seed_from_u64(effective_seed);
    lines.choose(&mut rng).expect("non-empty slice always yields Some")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_and_comments_ignored() {
        let content = "# comment\n\ne2e4 e7e5\n\n# another comment\nd2d4 d7d5\n";
        let lines = parse_opening_book(content);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].moves, vec!["e2e4", "e7e5"]);
        assert_eq!(lines[1].moves, vec!["d2d4", "d7d5"]);
    }

    #[test]
    fn test_parse_single_move_line() {
        let lines = parse_opening_book("e2e4\n");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].moves, vec!["e2e4"]);
    }

    #[test]
    fn test_parse_empty_input() {
        let lines = parse_opening_book("# only comments\n\n");
        assert!(lines.is_empty());
    }

    #[test]
    fn test_parse_multitoken_line() {
        let lines = parse_opening_book("e2e4 e7e5 g1f3 b8c6 f1b5\n");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].moves.len(), 5);
        assert_eq!(lines[0].moves[4], "f1b5");
    }

    #[test]
    fn test_validate_valid_lines() {
        let lines = parse_opening_book("e2e4 e7e5\nd2d4 d7d5 c2c4\n");
        let validated = validate_opening_book(lines).unwrap();
        assert_eq!(validated.len(), 2);
    }

    #[test]
    fn test_validate_invalid_move_errors() {
        // "e2e5" is not a legal pawn move from initial position
        let lines = parse_opening_book("e2e5\n");
        let result = validate_opening_book(lines);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("e2e5"));
    }

    #[test]
    fn test_validate_empty_lines_errors() {
        let lines = parse_opening_book("# no moves\n\n");
        let result = validate_opening_book(lines);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_select_same_seed_same_line() {
        let lines = parse_opening_book("e2e4 e7e5\nd2d4 d7d5\nc2c4\n");
        let validated = validate_opening_book(lines).unwrap();
        let a = select_opening_line(&validated, Some(42)).moves.clone();
        let b = select_opening_line(&validated, Some(42)).moves.clone();
        assert_eq!(a, b, "same seed must produce same line");
    }

    #[test]
    fn test_select_different_seeds_may_differ() {
        // With 3 lines and two very different seeds, it's overwhelmingly likely they differ.
        let lines = parse_opening_book("e2e4 e7e5\nd2d4 d7d5\nc2c4\n");
        let validated = validate_opening_book(lines).unwrap();
        // Just verify selection succeeds for two distinct seeds; we don't assert they differ
        // since that would be flaky for edge cases.
        let _ = select_opening_line(&validated, Some(1));
        let _ = select_opening_line(&validated, Some(u64::MAX));
    }

    #[test]
    fn test_select_no_seed_succeeds() {
        let lines = parse_opening_book("e2e4 e7e5\n");
        let validated = validate_opening_book(lines).unwrap();
        // Non-deterministic seed — just verify it returns a line without panicking.
        let line = select_opening_line(&validated, None);
        assert_eq!(line.moves, vec!["e2e4", "e7e5"]);
    }

    #[test]
    fn test_select_single_line_always_chosen() {
        let lines = parse_opening_book("e2e4 e7e5\n");
        let validated = validate_opening_book(lines).unwrap();
        for seed in [0u64, 1, 42, u64::MAX] {
            let chosen = select_opening_line(&validated, Some(seed));
            assert_eq!(chosen.moves, vec!["e2e4", "e7e5"]);
        }
    }
}
