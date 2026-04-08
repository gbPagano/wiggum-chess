use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::process::{Command, Output};

fn run_git_command(args: &[&str], cwd: Option<&Path>) -> Result<Output> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.output()
        .with_context(|| format!("spawning git {}", args.join(" ")))
}

fn git_failure(args: &[&str], output: &Output) -> anyhow::Error {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if !stderr.trim().is_empty() {
        stderr.trim().to_string()
    } else if !stdout.trim().is_empty() {
        stdout.trim().to_string()
    } else {
        "no output captured".to_string()
    };

    anyhow!(
        "git {} failed (exit {}): {}",
        args.join(" "),
        output.status,
        detail
    )
}

fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<()> {
    let output = run_git_command(args, cwd)?;
    if !output.status.success() {
        return Err(git_failure(args, &output));
    }
    Ok(())
}

fn run_git_output(args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let output = run_git_command(args, cwd)?;
    if !output.status.success() {
        return Err(git_failure(args, &output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_reports_changes(args: &[&str], cwd: &Path) -> Result<bool> {
    let output = run_git_command(args, Some(cwd))?;
    match output.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(git_failure(args, &output)),
    }
}

/// Returns the branch name used for a candidate worktree.
pub fn candidate_branch_name(session_id: &str, iteration: u32) -> String {
    format!("wiggum-evolution/{}/iteration-{}", session_id, iteration)
}

/// Creates a candidate git worktree at `candidate_dir` from `baseline_ref`.
pub fn create_candidate_workspace(
    repo_root: &Path,
    candidate_dir: &Path,
    baseline_ref: &str,
) -> Result<()> {
    let candidate_dir_str = candidate_dir
        .to_str()
        .ok_or_else(|| anyhow!("candidate_dir path is not valid UTF-8"))?;
    run_git(
        &["worktree", "add", "--detach", candidate_dir_str, baseline_ref],
        Some(repo_root),
    )
}

/// Creates the candidate branch inside an existing candidate worktree.
pub fn create_candidate_branch(candidate_dir: &Path, branch: &str) -> Result<()> {
    run_git(&["checkout", "-b", branch], Some(candidate_dir))
}

/// Removes a candidate git worktree. Does not error if the directory is missing.
pub fn remove_candidate_workspace(repo_root: &Path, candidate_dir: &Path) {
    if !candidate_dir.exists() {
        return;
    }

    if let Some(candidate_dir_str) = candidate_dir.to_str() {
        let _ = run_git(
            &["worktree", "remove", "--force", candidate_dir_str],
            Some(repo_root),
        );
    }
    let _ = std::fs::remove_dir_all(candidate_dir);
}

/// Returns true if the candidate workspace has uncommitted or untracked changes.
pub fn candidate_has_uncommitted_changes(candidate_dir: &Path) -> Result<bool> {
    if git_reports_changes(&["diff", "--quiet"], candidate_dir)? {
        return Ok(true);
    }

    if git_reports_changes(&["diff", "--cached", "--quiet"], candidate_dir)? {
        return Ok(true);
    }

    let output = run_git_output(
        &["ls-files", "--others", "--exclude-standard"],
        Some(candidate_dir),
    )?;
    Ok(!output.trim().is_empty())
}

/// Stages all changes and commits them in the candidate workspace.
pub fn commit_candidate_workspace(candidate_dir: &Path, iteration: u32) -> Result<()> {
    run_git(&["add", "-A"], Some(candidate_dir))?;
    run_git(
        &[
            "commit",
            "-m",
            &format!("chore: accept evolution iteration {}", iteration),
        ],
        Some(candidate_dir),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_branch_name_uses_session_and_iteration() {
        assert_eq!(
            candidate_branch_name("session-123", 7),
            "wiggum-evolution/session-123/iteration-7"
        );
    }
}
