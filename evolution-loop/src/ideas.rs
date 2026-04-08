use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::state::SessionMetadata;

const UNCHECKED_PATTERN: &str = "- [ ]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkResult {
    Updated,
    NotFound,
    Skipped,
}

/// Count lines matching `- [ ] <non-empty>` in the ideas file.
pub fn count_pending_ideas(path: &Path) -> Result<u32> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading ideas file {}", path.display()))?;
    let count = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with(UNCHECKED_PATTERN)
                && trimmed.len() > UNCHECKED_PATTERN.len()
                && !trimmed[UNCHECKED_PATTERN.len()..].trim().is_empty()
        })
        .count();
    Ok(count as u32)
}

/// Replace the first unchecked `- [ ] <selected_idea>` entry with `- [x] <selected_idea>`.
pub fn mark_idea_used(
    ideas_path: &Path,
    selected_idea: &str,
    proposal_source: &str,
) -> Result<MarkResult> {
    // Skip if path is empty or proposal source is not user_ideas_file
    if ideas_path.as_os_str().is_empty() || proposal_source != "user_ideas_file" {
        return Ok(MarkResult::Skipped);
    }

    let content = fs::read_to_string(ideas_path)
        .with_context(|| format!("reading ideas file {}", ideas_path.display()))?;

    let selected_trimmed = selected_idea.trim();
    let mut found = false;
    let new_content: String = content
        .lines()
        .map(|line| {
            if !found {
                let trimmed = line.trim();
                if trimmed.starts_with(UNCHECKED_PATTERN) {
                    let idea_text = trimmed[UNCHECKED_PATTERN.len()..].trim();
                    if idea_text == selected_trimmed {
                        found = true;
                        // Replace `[ ]` with `[x]` preserving indentation
                        return line.replacen("- [ ]", "- [x]", 1);
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

    // Preserve trailing newline if original had one
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
    if !path.exists() {
        return Ok(None);
    }
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
    if *mark_result != MarkResult::Updated {
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
    use tempfile::NamedTempFile;

    fn write_temp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn count_pending_ideas_counts_only_unchecked() {
        let content = "- [ ] idea one\n- [x] done idea\n- [ ] idea two\n# comment\n";
        let f = write_temp(content);
        let count = count_pending_ideas(f.path()).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn count_pending_ideas_ignores_checked() {
        let content = "- [x] already done\n- [x] also done\n";
        let f = write_temp(content);
        let count = count_pending_ideas(f.path()).unwrap();
        assert_eq!(count, 0);
    }
}
