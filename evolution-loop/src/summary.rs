use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::state::load_iteration_state;

pub fn write_placeholder_summary(path: &Path, session_id: &str) -> Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut f = fs::File::create(path)
        .with_context(|| format!("creating summary file {}", path.display()))?;
    writeln!(f, "# Evolution Session Summary")?;
    writeln!(f, "")?;
    writeln!(f, "Session ID: {}", session_id)?;
    writeln!(f, "Status: pending final session summary")?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn write_session_summary(
    path: &Path,
    session_dir: &Path,
    session_id: &str,
    baseline_version: &str,
    max_iterations: u32,
    completed_iterations: u32,
    stop_reason: &str,
    stop_reason_details: &str,
    accepted_baseline_version: &str,
    accepted_baseline_path: &str,
) -> Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;

    // Collect per-iteration data
    let iterations_dir = session_dir.join("iterations");
    let mut iteration_rows: Vec<String> = vec![];
    let mut accepted_versions: Vec<String> = vec![];
    let mut rejected_versions: Vec<String> = vec![];

    if iterations_dir.exists() {
        let mut entries: Vec<_> = fs::read_dir(&iterations_dir)
            .context("reading iterations directory")?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        entries.sort_by_key(|e| {
            e.file_name()
                .to_string_lossy()
                .parse::<u32>()
                .unwrap_or(u32::MAX)
        });

        for entry in entries {
            let iter_dir = entry.path();
            let iter_json = iter_dir.join("iteration.json");
            if !iter_json.exists() {
                continue;
            }
            let iter_state = match load_iteration_state(&iter_json) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let n = iter_state.iteration;
            let outcome = iter_state
                .decision
                .as_ref()
                .map(|d| d.outcome.as_str())
                .unwrap_or("unknown");

            let row = format!(
                "| {} | {} | {} | [iteration.json]({p}/iteration.json) | [hypothesis.md]({p}/hypothesis.md) | [implementation.md]({p}/implementation.md) | [correctness/results.md]({p}/correctness/results.md) | [benchmark.md]({p}/benchmark.md) | [stockfish-comparison/results.md]({p}/stockfish-comparison/results.md) | [decision.md]({p}/decision.md) |",
                n,
                iter_state.candidate.version.as_deref().unwrap_or("-"),
                outcome,
                p = iter_dir.display(),
            );
            iteration_rows.push(row);

            if outcome == "accepted" {
                if let Some(v) = iter_state.candidate.version.as_deref() {
                    accepted_versions.push(format!("- v{} (iteration {})", v, n));
                }
            } else {
                rejected_versions.push(format!("- Iteration {} ({})", n, outcome));
            }
        }
    }

    let mut f = fs::File::create(path)
        .with_context(|| format!("creating summary file {}", path.display()))?;

    writeln!(f, "# Evolution Session Summary")?;
    writeln!(f)?;
    writeln!(f, "| Field | Value |")?;
    writeln!(f, "|-------|-------|")?;
    writeln!(f, "| Session ID | {} |", session_id)?;
    writeln!(f, "| Initial Baseline | {} |", baseline_version)?;
    writeln!(f, "| Final Accepted Baseline | {} |", accepted_baseline_version)?;
    writeln!(f, "| Accepted Baseline Path | {} |", accepted_baseline_path)?;
    writeln!(f, "| Max Iterations | {} |", max_iterations)?;
    writeln!(f, "| Completed Iterations | {} |", completed_iterations)?;
    writeln!(f, "| Stop Reason | {} |", stop_reason)?;
    writeln!(f, "| Stop Details | {} |", stop_reason_details)?;
    writeln!(f, "| Session Directory | {} |", session_dir.display())?;
    writeln!(f)?;

    writeln!(f, "## Accepted Versions")?;
    writeln!(f)?;
    if accepted_versions.is_empty() {
        writeln!(f, "_No versions accepted._")?;
    } else {
        for v in &accepted_versions {
            writeln!(f, "{}", v)?;
        }
    }
    writeln!(f)?;

    writeln!(f, "## Rejected / Inconclusive Attempts")?;
    writeln!(f)?;
    if rejected_versions.is_empty() {
        writeln!(f, "_None._")?;
    } else {
        for v in &rejected_versions {
            writeln!(f, "{}", v)?;
        }
    }
    writeln!(f)?;

    writeln!(f, "## Per-Iteration Artifacts")?;
    writeln!(f)?;
    writeln!(f, "| N | Candidate | Outcome | State | Hypothesis | Implementation | Correctness | Benchmark | Stockfish Comparison | Decision |")?;
    writeln!(f, "|---|-----------|---------|-------|------------|----------------|-------------|-----------|----------------------|----------|")?;
    for row in &iteration_rows {
        writeln!(f, "{}", row)?;
    }

    Ok(())
}
