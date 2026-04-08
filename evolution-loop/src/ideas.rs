use anyhow::{Context, Result};
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::state::SessionMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkResult {
    Updated,
    NotFound,
    Skipped,
}

fn parse_unchecked_idea_line(line: &str) -> Option<(Range<usize>, &str)> {
    let trimmed_start = line.trim_start();
    let after_dash = trimmed_start.strip_prefix('-')?;
    let after_dash = after_dash.trim_start();
    let bracket_index = line.len() - after_dash.len();
    let after_checkbox = after_dash.strip_prefix("[ ]")?;

    if !after_checkbox.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }

    let idea_text = after_checkbox.trim();
    if idea_text.is_empty() {
        return None;
    }

    Some((bracket_index..bracket_index + 3, idea_text))
}

/// Count lines matching `^\s*-\s*\[ \]\s+.+` in the ideas file.
pub fn count_pending_ideas(path: &Path) -> Result<u32> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading ideas file {}", path.display()))?;
    let count = content
        .lines()
        .filter(|line| parse_unchecked_idea_line(line).is_some())
        .count();
    Ok(count as u32)
}

/// Replace the first unchecked checklist entry matching `selected_idea`.
pub fn mark_idea_used(
    ideas_path: &Path,
    selected_idea: &str,
    proposal_source: &str,
) -> Result<MarkResult> {
    if ideas_path.as_os_str().is_empty() || proposal_source != "user_ideas_file" {
        return Ok(MarkResult::Skipped);
    }

    let content = fs::read_to_string(ideas_path)
        .with_context(|| format!("reading ideas file {}", ideas_path.display()))?;

    let selected_trimmed = selected_idea.trim();
    let mut found = false;
    let new_content = content
        .lines()
        .map(|line| {
            if !found {
                if let Some((checkbox_range, idea_text)) = parse_unchecked_idea_line(line) {
                    if idea_text == selected_trimmed {
                        found = true;
                        let mut updated_line = line.to_string();
                        updated_line.replace_range(checkbox_range, "[x]");
                        return updated_line;
                    }
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");

    if !found {
        return Ok(MarkResult::NotFound);
    }

    let final_content = if content.ends_with('\n') {
        format!("{}\n", new_content)
    } else {
        new_content
    };

    fs::write(ideas_path, final_content)
        .with_context(|| format!("writing ideas file {}", ideas_path.display()))?;

    Ok(MarkResult::Updated)
}

/// Resolve an ideas file path. Returns None if the file has zero pending ideas.
pub fn resolve_ideas_file(raw_path: &str, repo_root: &Path) -> Result<Option<PathBuf>> {
    if raw_path.is_empty() {
        return Ok(None);
    }

    let path = if Path::new(raw_path).is_absolute() {
        PathBuf::from(raw_path)
    } else {
        repo_root.join(raw_path)
    };

    let pending = count_pending_ideas(&path)?;
    if pending == 0 {
        return Ok(None);
    }

    Ok(Some(path))
}

/// After marking an idea used, update session metadata pending count and
/// optionally clear the ideas_file field if all ideas are exhausted.
pub fn update_session_after_mark(
    meta: &mut SessionMetadata,
    ideas_path: &Path,
    mark_result: &MarkResult,
) -> Result<()> {
    if *mark_result == MarkResult::Skipped {
        return Ok(());
    }

    let pending = count_pending_ideas(ideas_path)?;
    meta.ideas_file_pending_count = pending.to_string();
    if pending == 0 {
        meta.ideas_file = String::new();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    fn write_temp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn count_pending_ideas_counts_only_unchecked() {
        let content = "- [ ] idea one\n- [x] done idea\n -    [ ] idea two\n# comment\n";
        let f = write_temp(content);
        let count = count_pending_ideas(f.path()).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn count_pending_ideas_ignores_invalid_and_checked_lines() {
        let content = "- [x] already done\n- [ ]\n- [ ]    \nnot a checklist\n";
        let f = write_temp(content);
        let count = count_pending_ideas(f.path()).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn mark_idea_used_updates_first_matching_unchecked_entry() {
        let f = write_temp("  -   [ ]  tune pruning\n- [ ] tune pruning\n");

        let result = mark_idea_used(f.path(), "tune pruning", "user_ideas_file").unwrap();

        assert_eq!(result, MarkResult::Updated);
        let content = fs::read_to_string(f.path()).unwrap();
        assert_eq!(content, "  -   [x]  tune pruning\n- [ ] tune pruning\n");
    }

    #[test]
    fn mark_idea_used_skips_non_checklist_sources() {
        let f = write_temp("- [ ] tune pruning\n");

        let result = mark_idea_used(f.path(), "tune pruning", "self_proposed").unwrap();

        assert_eq!(result, MarkResult::Skipped);
        let content = fs::read_to_string(f.path()).unwrap();
        assert_eq!(content, "- [ ] tune pruning\n");
    }

    #[test]
    fn resolve_ideas_file_returns_none_when_no_pending_entries() {
        let f = write_temp("- [x] done\n");

        let resolved = resolve_ideas_file(&f.path().to_string_lossy(), Path::new("/workspace"))
            .unwrap();

        assert!(resolved.is_none());
    }

    #[test]
    fn resolve_ideas_file_resolves_relative_paths() {
        let tempdir = TempDir::new().unwrap();
        let ideas_path = tempdir.path().join("ideas.md");
        fs::write(&ideas_path, "- [ ] idea one\n").unwrap();

        let resolved = resolve_ideas_file("ideas.md", tempdir.path()).unwrap();

        assert_eq!(resolved, Some(ideas_path));
    }

    #[test]
    fn update_session_after_mark_clears_ideas_file_when_exhausted() {
        let f = write_temp("- [ ] idea one\n");
        mark_idea_used(f.path(), "idea one", "user_ideas_file").unwrap();

        let mut meta = SessionMetadata {
            ideas_file: f.path().to_string_lossy().to_string(),
            ideas_file_pending_count: "1".to_string(),
            ..SessionMetadata::default()
        };

        update_session_after_mark(&mut meta, f.path(), &MarkResult::Updated).unwrap();

        assert_eq!(meta.ideas_file_pending_count, "0");
        assert!(meta.ideas_file.is_empty());
    }
}
