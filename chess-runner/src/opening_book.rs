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
}
