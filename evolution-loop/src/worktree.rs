use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let output = cmd
        .output()
        .with_context(|| format!("spawning git {}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git {} failed (exit {}): {}",
            args.join(" "),
            output.status,
            stderr.trim()
        );
    }
    Ok(())
}

fn run_git_output(args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let output = cmd
        .output()
        .with_context(|| format!("spawning git {}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git {} failed (exit {}): {}",
            args.join(" "),
            output.status,
            stderr.trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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
        .ok_or_else(|| anyhow::anyhow!("candidate_dir path is not valid UTF-8"))?;
    run_git(
        &["worktree", "add", "--detach", candidate_dir_str, baseline_ref],
        Some(repo_root),
    )?;
    Ok(())
}

/// Removes a candidate git worktree. Does not error if the directory is missing.
pub fn remove_candidate_workspace(repo_root: &Path, candidate_dir: &Path) {
    if !candidate_dir.exists() {
        return;
    }
    let candidate_dir_str = match candidate_dir.to_str() {
        Some(s) => s,
        None => return,
    };
    let _ = run_git(
        &["worktree", "remove", "--force", candidate_dir_str],
        Some(repo_root),
    );
    let _ = std::fs::remove_dir_all(candidate_dir);
}

/// Returns true if the candidate workspace has uncommitted or untracked changes.
pub fn candidate_has_uncommitted_changes(candidate_dir: &Path) -> Result<bool> {
    let mut cmd = Command::new("git");
    cmd.args(["diff", "--quiet"]).current_dir(candidate_dir);
    let status = cmd
        .status()
        .context("running git diff --quiet")?;
    if !status.success() {
        return Ok(true);
    }

    let mut cmd = Command::new("git");
    cmd.args(["diff", "--cached", "--quiet"])
        .current_dir(candidate_dir);
    let status = cmd
        .status()
        .context("running git diff --cached --quiet")?;
    if !status.success() {
        return Ok(true);
    }

    // Check for untracked files
    let output = run_git_output(
        &["ls-files", "--others", "--exclude-standard"],
        Some(candidate_dir),
    )?;
    if !output.trim().is_empty() {
        return Ok(true);
    }

    Ok(false)
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
    )?;
    Ok(())
}
