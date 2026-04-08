mod artifacts;
mod correctness;
mod ideas;
mod phase;
mod promotion;
mod state;
mod summary;
mod versioning;
mod worktree;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use std::fs;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};

use artifacts::create_iteration_artifacts;
use correctness::{run_correctness_gate, CorrectnessOutcome};
use ideas::{mark_idea_used, resolve_ideas_file, update_session_after_mark, MarkResult};
use phase::{run_phase, PhaseConfig, PhaseOutcome};
use promotion::promote_candidate;
use state::{
    load_iteration_state, load_session_metadata, save_session_metadata, SessionMetadata,
};
use summary::{write_placeholder_summary, write_session_summary};
use versioning::{
    apply_candidate_manifest_versions, candidate_version_for_source, cargo_semver_from_tag,
    parse_version_tag, ProposalSource,
};
use worktree::{
    candidate_branch_name, create_candidate_branch, create_candidate_workspace,
    remove_candidate_workspace,
};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "evolution-loop", about = "Chess engine evolution orchestrator")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a new evolution session
    Start {
        /// Baseline version tag (e.g. v1.2)
        #[arg(long)]
        baseline_version: String,

        /// Optional ideas checklist file path
        #[arg(long)]
        ideas_file: Option<PathBuf>,

        /// Output directory for session artifacts
        #[arg(long, default_value = "tasks/evolution-runs")]
        output_dir: PathBuf,

        /// Maximum number of iterations
        #[arg(long, default_value_t = 10)]
        max_iterations: u32,

        /// Maximum consecutive infra failures before stopping
        #[arg(long, default_value_t = 3)]
        max_infra_failures: u32,

        /// Per-phase timeout in seconds
        #[arg(long, default_value_t = 1800)]
        phase_timeout_secs: u64,

        /// Stream Claude phase output to stdout in addition to log files
        #[arg(long)]
        verbose: bool,
    },
    /// Resume an interrupted evolution session
    Resume {
        /// Path to the session directory (containing session.env)
        #[arg(long)]
        session: PathBuf,

        /// Resume from a specific phase (propose/implement/validate/benchmark/decide)
        #[arg(long, value_parser = parse_phase_arg)]
        from: Option<String>,

        /// Per-phase timeout in seconds
        #[arg(long, default_value_t = 1800)]
        phase_timeout_secs: u64,

        /// Stream Claude phase output to stdout in addition to log files
        #[arg(long)]
        verbose: bool,
    },
}

fn parse_phase_arg(s: &str) -> std::result::Result<String, String> {
    match s {
        "propose" | "implement" | "validate" | "benchmark" | "decide" => Ok(s.to_string()),
        _ => Err(format!(
            "invalid phase '{}': must be one of propose, implement, validate, benchmark, decide",
            s
        )),
    }
}

// ---------------------------------------------------------------------------
// Session config (runtime)
// ---------------------------------------------------------------------------

struct SessionConfig {
    repo_root: PathBuf,
    session_dir: PathBuf,
    session_env_path: PathBuf,
    summary_path: PathBuf,
    phase_timeout_secs: u64,
    verbose: bool,
    max_iterations: u32,
    max_infra_failures: u32,
    stop_flag: Arc<AtomicBool>,
}

// ---------------------------------------------------------------------------
// Phase helpers
// ---------------------------------------------------------------------------

/// Records a phase failure into iteration.json, benchmark.md, and decision.md.
fn record_phase_failure(
    iteration_json: &Path,
    decision_md: &Path,
    benchmark_md: &Path,
    phase: &str,
    reason: &str,
) -> Result<()> {
    // Update iteration.json using raw Value to avoid struct limitations
    let content = fs::read_to_string(iteration_json)
        .with_context(|| format!("reading {}", iteration_json.display()))?;
    let mut data: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("parsing {}", iteration_json.display()))?;

    data["state"] = serde_json::json!("failed");
    data["stateMachine"]["current"] = serde_json::json!("failed");

    if data["decision"].is_null() || !data["decision"].is_object() {
        data["decision"] = serde_json::json!({});
    }
    data["decision"]["outcome"] = serde_json::json!("failed");
    data["decision"]["reasoning"] = serde_json::json!(reason);
    data["decision"]["evidence"] = serde_json::json!([phase]);

    if phase == "propose" {
        data["hypothesis"] = serde_json::json!({
            "status": "failed",
            "summary": "Hypothesis generation failed.",
            "failureReason": reason,
            "targetMetrics": [],
            "buildsOn": []
        });
        data["benchmark"] = serde_json::json!({
            "status": "skipped",
            "skippedReason": "proposal phase failed"
        });
    } else if phase == "implement" {
        data["implementation"] = serde_json::json!({
            "summary": "Implementation phase failed before a candidate was completed.",
            "failureReason": reason,
            "changedFiles": []
        });
        data["benchmark"] = serde_json::json!({
            "status": "skipped",
            "skippedReason": "implementation phase failed"
        });
    } else if phase == "benchmark" {
        data["benchmark"] = serde_json::json!({
            "status": "failed",
            "failureReason": reason,
            "sufficientForPromotion": false
        });
    }

    let updated = serde_json::to_string_pretty(&data).context("serializing iteration state")?;
    let mut f = fs::File::create(iteration_json)
        .with_context(|| format!("writing {}", iteration_json.display()))?;
    writeln!(f, "{}", updated)?;

    // Write benchmark.md
    if phase == "propose" || phase == "implement" {
        let mut f = fs::File::create(benchmark_md)
            .with_context(|| format!("writing {}", benchmark_md.display()))?;
        writeln!(f, "# Iteration Benchmark")?;
        writeln!(f)?;
        writeln!(f, "Status: skipped")?;
        writeln!(f)?;
        writeln!(f, "Benchmark execution is skipped because the {} phase failed.", phase)?;
        writeln!(f)?;
        writeln!(f, "Reason: {}", reason)?;
    } else if phase == "benchmark" {
        let mut f = fs::File::create(benchmark_md)
            .with_context(|| format!("writing {}", benchmark_md.display()))?;
        writeln!(f, "# Iteration Benchmark")?;
        writeln!(f)?;
        writeln!(f, "Status: failed")?;
        writeln!(f)?;
        writeln!(f, "Benchmark execution failed.")?;
        writeln!(f)?;
        writeln!(f, "Reason: {}", reason)?;
    }

    // Write decision.md
    {
        let mut f = fs::File::create(decision_md)
            .with_context(|| format!("writing {}", decision_md.display()))?;
        writeln!(f, "# Iteration Decision")?;
        writeln!(f)?;
        writeln!(f, "Status: failed")?;
        writeln!(f)?;
        writeln!(f, "The {} phase failed.", phase)?;
        writeln!(f)?;
        writeln!(f, "Reason: {}", reason)?;
    }

    Ok(())
}

/// Checks the `state` field in iteration.json against an expected phase name.
fn check_iteration_state_is(iteration_json: &Path, _expected: &str) -> Result<String> {
    let content = fs::read_to_string(iteration_json)
        .with_context(|| format!("reading {}", iteration_json.display()))?;
    let data: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("parsing {}", iteration_json.display()))?;
    let current = data["state"].as_str().unwrap_or("").to_string();
    Ok(current)
}

/// Returns true if iteration.json contains a "no_hypothesis" signal in any of the
/// state, hypothesis.status, hypothesis.state, or hypothesis.stopSignal fields.
fn iteration_has_no_hypothesis(iteration_json: &Path) -> bool {
    let Ok(content) = fs::read_to_string(iteration_json) else { return false };
    let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) else { return false };

    let hypothesis = data.get("hypothesis").and_then(|h| h.as_object());
    let signals = [
        data["state"].as_str().unwrap_or(""),
        hypothesis
            .and_then(|h| h.get("status"))
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        hypothesis
            .and_then(|h| h.get("state"))
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        hypothesis
            .and_then(|h| h.get("stopSignal"))
            .and_then(|v| v.as_str())
            .unwrap_or(""),
    ];
    signals.iter().any(|s| *s == "no_hypothesis")
}

/// Copies the built chess-engine binary to the wiggum-engine path in the candidate workspace.
fn copy_candidate_binary(candidate_dir: &Path) -> Result<PathBuf> {
    let source = candidate_dir.join("target").join("debug").join("chess-engine");
    let dest = candidate_dir.join("target").join("debug").join("wiggum-engine");

    if !source.exists() {
        anyhow::bail!(
            "candidate binary not found after build: {}",
            source.display()
        );
    }

    debug!(src = %source.display(), dst = %dest.display(), "copying candidate binary");
    fs::copy(&source, &dest)
        .with_context(|| format!("copying {} to {}", source.display(), dest.display()))?;

    // Set executable bit
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dest)?.permissions();
        perms.set_mode(perms.mode() | 0o111);
        fs::set_permissions(&dest, perms)?;
    }

    Ok(dest)
}

// ---------------------------------------------------------------------------
// Iteration runner
// ---------------------------------------------------------------------------

struct IterationResult {
    outcome: String, // accepted / rejected / inconclusive / failed
    infra_failure: bool,
    /// Set when the loop should stop after this iteration (reason, details).
    stop_session: Option<(String, String)>,
}

fn run_iteration(
    n: u32,
    cfg: &SessionConfig,
    meta: &mut SessionMetadata,
) -> Result<IterationResult> {
    info!(iteration = n, "starting iteration");

    let branch = candidate_branch_name(&meta.session_id, n);
    let candidate_dir = cfg
        .session_dir
        .join("candidate-workspaces")
        .join(format!("iteration-{}", n));

    // Set up git worktree
    let (setup_status, setup_error) = match create_candidate_workspace(
        &cfg.repo_root,
        &candidate_dir,
        &meta.active_baseline_version,
    ) {
        Ok(()) => match create_candidate_branch(&candidate_dir, &branch) {
            Ok(()) => ("ok".to_string(), String::new()),
            Err(e) => {
                warn!(iteration = n, error = %e, "candidate branch creation failed");
                remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
                ("failed".to_string(), e.to_string())
            }
        },
        Err(e) => {
            warn!(iteration = n, error = %e, "candidate workspace setup failed");
            ("failed".to_string(), e.to_string())
        }
    };

    // Set provisional candidate metadata (will be refined after propose)
    let provisional_version =
        candidate_version_for_source(&meta.active_baseline_version, ProposalSource::SelfProposed)
            .unwrap_or_else(|_| meta.active_baseline_version.clone());
    meta.candidate_version = provisional_version;
    meta.candidate_binary_path = candidate_dir
        .join("target")
        .join("debug")
        .join("wiggum-engine")
        .to_string_lossy()
        .to_string();
    save_session_metadata(&cfg.session_env_path, meta)?;

    let paths = create_iteration_artifacts(
        &cfg.session_dir,
        n,
        meta,
        &candidate_dir,
        &branch,
        &setup_status,
        &setup_error,
    )?;

    if setup_status == "failed" {
        return Ok(IterationResult {
            outcome: "failed".to_string(),
            infra_failure: true,
            stop_session: None,
        });
    }

    // Phase runner helper
    let make_phase_config = |skill: &str| PhaseConfig {
        skill_name: skill.to_string(),
        candidate_workspace: candidate_dir.clone(),
        iteration_dir: paths.iteration_dir.clone(),
        iteration_state_path: paths.iteration_json.clone(),
        session_dir: cfg.session_dir.clone(),
        repo_root: cfg.repo_root.clone(),
        session_metadata_path: cfg.session_env_path.clone(),
        phase_timeout_secs: cfg.phase_timeout_secs,
        verbose: cfg.verbose,
    };

    // --- propose ---
    info!(iteration = n, phase = "propose", "running phase");
    match run_phase(&make_phase_config("evolution-propose"))? {
        PhaseOutcome::Timeout => {
            warn!(iteration = n, phase = "propose", timeout_secs = cfg.phase_timeout_secs, "phase timed out");
            let reason = "Claude skill execution timed out during the propose phase.";
            let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "propose", reason);
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
        }
        PhaseOutcome::Failed(code) => {
            warn!(iteration = n, phase = "propose", exit_code = code, "phase failed");
            let reason = "Claude skill execution failed during the propose phase.";
            let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "propose", reason);
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
        }
        PhaseOutcome::Success => {
            info!(iteration = n, phase = "propose", outcome = "success", "phase complete");
        }
    }

    // Check for no-hypothesis stop condition
    if iteration_has_no_hypothesis(&paths.iteration_json) {
        info!(iteration = n, "propose phase produced no_hypothesis stop signal; stopping session");
        remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
        return Ok(IterationResult {
            outcome: "failed".to_string(),
            infra_failure: false,
            stop_session: Some((
                "no valid next hypothesis could be generated".to_string(),
                format!("iteration {} returned a no_hypothesis stop signal", n),
            )),
        });
    }

    // Verify proposal_source is valid
    let iter_state_after_propose = load_iteration_state(&paths.iteration_json)?;
    let proposal_source_str = iter_state_after_propose
        .ideas
        .proposal_source
        .clone()
        .unwrap_or_default();
    if proposal_source_str != "user_ideas_file" && proposal_source_str != "self_proposed" {
        let reason = "Proposal phase completed without writing ideas.proposalSource as 'user_ideas_file' or 'self_proposed'.";
        warn!(iteration = n, phase = "propose", proposal_source = %proposal_source_str, "{}", reason);
        let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "propose", reason);
        remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
        return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
    }

    // Verify state == "proposed"
    let current_state = check_iteration_state_is(&paths.iteration_json, "proposed")?;
    if current_state != "proposed" {
        let reason = format!(
            "Proposal phase completed without writing state 'proposed' (got '{}').",
            current_state
        );
        warn!(iteration = n, phase = "propose", current_state = %current_state, "unexpected state after propose");
        let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "propose", &reason);
        remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
        return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
    }

    // Now that we know the proposal source, compute the real candidate version
    let proposal_source_enum = if proposal_source_str == "user_ideas_file" {
        ProposalSource::UserIdeasFile
    } else {
        ProposalSource::SelfProposed
    };
    let candidate_version =
        match candidate_version_for_source(&meta.active_baseline_version, proposal_source_enum) {
            Ok(v) => v,
            Err(e) => {
                let reason = format!("Failed to compute candidate version: {}", e);
                warn!(iteration = n, error = %e, "candidate versioning failed");
                let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "implement", &reason);
                remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
                return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
            }
        };

    // Apply Cargo manifest versions in candidate workspace
    match cargo_semver_from_tag(&candidate_version)
        .and_then(|semver| apply_candidate_manifest_versions(&candidate_dir, &semver))
    {
        Ok(()) => {
            debug!(iteration = n, candidate_version = %candidate_version, "applied candidate manifest versions");
        }
        Err(e) => {
            let reason = format!("Failed to apply candidate version metadata before implementation: {}", e);
            warn!(iteration = n, error = %e, "manifest version apply failed");
            let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "implement", &reason);
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
        }
    }

    // Update session metadata with real candidate version
    meta.candidate_version = candidate_version.clone();
    meta.candidate_binary_path = candidate_dir
        .join("target")
        .join("debug")
        .join("wiggum-engine")
        .to_string_lossy()
        .to_string();
    save_session_metadata(&cfg.session_env_path, meta)?;

    // --- implement ---
    info!(iteration = n, phase = "implement", "running phase");
    match run_phase(&make_phase_config("evolution-implement"))? {
        PhaseOutcome::Timeout => {
            warn!(iteration = n, phase = "implement", timeout_secs = cfg.phase_timeout_secs, "phase timed out");
            let reason = "Claude skill execution timed out during the implementation phase.";
            let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "implement", reason);
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
        }
        PhaseOutcome::Failed(code) => {
            warn!(iteration = n, phase = "implement", exit_code = code, "phase failed");
            let reason = "Claude skill execution failed during the implementation phase.";
            let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "implement", reason);
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
        }
        PhaseOutcome::Success => {
            info!(iteration = n, phase = "implement", outcome = "success", "phase complete");
        }
    }

    // Verify state after implement
    let current_state = check_iteration_state_is(&paths.iteration_json, "implemented")?;
    if current_state == "failed" {
        info!(iteration = n, phase = "implement", "iteration marked failed by implement phase");
        remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
        return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: false, stop_session: None });
    }
    if current_state != "implemented" {
        let reason = format!(
            "Implementation phase completed without writing state 'implemented' (got '{}').",
            current_state
        );
        warn!(iteration = n, phase = "implement", current_state = %current_state, "unexpected state after implement");
        let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "implement", &reason);
        remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
        return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
    }

    // --- validate (correctness gate) ---
    info!(iteration = n, phase = "validate", "running correctness gate");
    let correctness_outcome = run_correctness_gate(
        &candidate_dir,
        &paths.iteration_json,
        &paths.correctness_results_md,
        cfg.phase_timeout_secs,
    )?;
    match &correctness_outcome {
        CorrectnessOutcome::Passed => {
            info!(iteration = n, phase = "validate", outcome = "passed", "correctness gate passed");
        }
        CorrectnessOutcome::Failed(_) => {
            warn!(iteration = n, phase = "validate", "correctness gate failed");
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: false, stop_session: None });
        }
    }

    // Verify state is still "implemented" after correctness gate
    let current_state = check_iteration_state_is(&paths.iteration_json, "implemented")?;
    if current_state != "implemented" {
        let reason = format!(
            "Correctness gate returned an unexpected state before benchmarking (got '{}').",
            current_state
        );
        warn!(iteration = n, phase = "validate", current_state = %current_state, "unexpected state after correctness gate");
        let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "benchmark", &reason);
        remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
        return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
    }

    // --- copy binary ---
    info!(iteration = n, "copying candidate binary");
    match copy_candidate_binary(&candidate_dir) {
        Ok(dest) => {
            debug!(iteration = n, dest = %dest.display(), "candidate binary copied");
        }
        Err(e) => {
            let reason = format!(
                "Implementation phase completed but the candidate binary could not be built at {}: {}",
                candidate_dir.join("target").join("debug").join("wiggum-engine").display(),
                e
            );
            warn!(iteration = n, error = %e, "candidate binary copy failed");
            let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "implement", &reason);
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: false, stop_session: None });
        }
    }

    // --- benchmark ---
    info!(iteration = n, phase = "benchmark", "running phase");
    match run_phase(&make_phase_config("evolution-benchmark"))? {
        PhaseOutcome::Timeout => {
            warn!(iteration = n, phase = "benchmark", timeout_secs = cfg.phase_timeout_secs, "phase timed out");
            let reason = "Claude skill execution timed out during the benchmark phase.";
            let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "benchmark", reason);
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
        }
        PhaseOutcome::Failed(code) => {
            warn!(iteration = n, phase = "benchmark", exit_code = code, "phase failed");
            let reason = "Claude skill execution failed during the benchmark phase.";
            let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "benchmark", reason);
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
        }
        PhaseOutcome::Success => {
            info!(iteration = n, phase = "benchmark", outcome = "success", "phase complete");
        }
    }

    // Verify state == "benchmarked"
    let current_state = check_iteration_state_is(&paths.iteration_json, "benchmarked")?;
    if current_state == "failed" {
        info!(iteration = n, phase = "benchmark", "iteration marked failed by benchmark phase");
        remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
        return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: false, stop_session: None });
    }
    if current_state != "benchmarked" {
        let reason = format!(
            "Benchmark phase completed without writing state 'benchmarked' (got '{}').",
            current_state
        );
        warn!(iteration = n, phase = "benchmark", current_state = %current_state, "unexpected state after benchmark");
        let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "benchmark", &reason);
        remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
        return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
    }

    // --- decide ---
    info!(iteration = n, phase = "decide", "running phase");
    match run_phase(&make_phase_config("evolution-decide"))? {
        PhaseOutcome::Timeout => {
            warn!(iteration = n, phase = "decide", timeout_secs = cfg.phase_timeout_secs, "phase timed out");
            let reason = "Claude skill execution timed out during the decision phase.";
            let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "decision", reason);
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
        }
        PhaseOutcome::Failed(code) => {
            warn!(iteration = n, phase = "decide", exit_code = code, "phase failed");
            let reason = "Claude skill execution failed during the decision phase.";
            let _ = record_phase_failure(&paths.iteration_json, &paths.decision_md, &paths.benchmark_md, "decision", reason);
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true, stop_session: None });
        }
        PhaseOutcome::Success => {
            info!(iteration = n, phase = "decide", outcome = "success", "phase complete");
        }
    }

    // Read final iteration state to get outcome
    let final_state = load_iteration_state(&paths.iteration_json)?;
    let outcome = final_state
        .decision
        .as_ref()
        .map(|d| d.outcome.clone())
        .unwrap_or_else(|| "inconclusive".to_string());

    // Handle outcome
    let proposal_source_final = final_state
        .ideas
        .proposal_source
        .clone()
        .unwrap_or_default();
    let selected_idea = final_state
        .ideas
        .selected_idea
        .clone()
        .unwrap_or_default();

    let infra_failure = match outcome.as_str() {
        "accepted" => {
            // Mark idea used
            if let Ok(ideas_path) = resolve_ideas_file(&meta.ideas_file, &cfg.repo_root) {
                if let Some(p) = ideas_path {
                    let mark_result =
                        mark_idea_used(&p, &selected_idea, &proposal_source_final)
                            .unwrap_or(MarkResult::Skipped);
                    let _ = update_session_after_mark(meta, &p, &mark_result);
                }
            }
            // Promote
            let promotion_ok = promote_candidate(
                &candidate_dir,
                &paths.iteration_json,
                meta,
                &cfg.repo_root,
            )
            .is_ok();
            if !promotion_ok {
                warn!(iteration = n, "promotion failed");
            }
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            save_session_metadata(&cfg.session_env_path, meta)?;
            // Reset infra failures on success, increment on promotion failure
            !promotion_ok
        }
        "rejected" | "inconclusive" => {
            // Mark idea used
            let mut infra = false;
            if let Ok(ideas_path) = resolve_ideas_file(&meta.ideas_file, &cfg.repo_root) {
                if let Some(p) = ideas_path {
                    let mark_result =
                        mark_idea_used(&p, &selected_idea, &proposal_source_final)
                            .unwrap_or(MarkResult::NotFound);
                    if mark_result == MarkResult::NotFound {
                        infra = true;
                    }
                    let _ = update_session_after_mark(meta, &p, &mark_result);
                }
            }
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            save_session_metadata(&cfg.session_env_path, meta)?;
            infra
        }
        _ => {
            // failed or unexpected
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            true
        }
    };

    info!(iteration = n, outcome = %outcome, "iteration complete");
    Ok(IterationResult { outcome, infra_failure, stop_session: None })
}

// ---------------------------------------------------------------------------
// Start flow
// ---------------------------------------------------------------------------

fn run_start(
    baseline_version: String,
    ideas_file: Option<PathBuf>,
    output_dir: PathBuf,
    max_iterations: u32,
    max_infra_failures: u32,
    phase_timeout_secs: u64,
    verbose: bool,
) -> Result<()> {
    // Validate baseline version format
    parse_version_tag(&baseline_version)
        .with_context(|| format!("invalid --baseline-version '{}'", baseline_version))?;

    let repo_root = std::env::current_dir()?;

    // Validate baseline artifacts
    let baseline_dir = repo_root
        .join("chess-engine")
        .join("versions")
        .join(&baseline_version);
    if !baseline_dir.exists() {
        anyhow::bail!(
            "baseline version directory does not exist: {}",
            baseline_dir.display()
        );
    }
    let baseline_binary = baseline_dir.join("wiggum-engine");
    if !baseline_binary.exists() {
        anyhow::bail!(
            "baseline binary does not exist: {}",
            baseline_binary.display()
        );
    }

    // Generate session ID
    let session_id = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let session_dir = output_dir.join(&session_id);
    std::fs::create_dir_all(&session_dir)
        .with_context(|| format!("creating session directory {}", session_dir.display()))?;

    let session_env_path = session_dir.join("session.env");
    let summary_path = session_dir.join("summary.md");

    // Resolve ideas file
    let resolved_ideas = ideas_file
        .as_ref()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    let resolved_ideas_path = ideas::resolve_ideas_file(resolved_ideas, &repo_root)?;
    let (ideas_file_str, ideas_pending_count) = if let Some(p) = &resolved_ideas_path {
        let count = ideas::count_pending_ideas(p)?;
        (p.to_string_lossy().to_string(), count.to_string())
    } else {
        (String::new(), "0".to_string())
    };

    let meta = SessionMetadata {
        baseline_version: baseline_version.clone(),
        active_baseline_version: baseline_version.clone(),
        active_baseline_path: baseline_dir.to_string_lossy().to_string(),
        active_baseline_binary: baseline_binary.to_string_lossy().to_string(),
        accepted_baseline_version: baseline_version.clone(),
        accepted_baseline_path: baseline_dir.to_string_lossy().to_string(),
        accepted_baseline_binary: baseline_binary.to_string_lossy().to_string(),
        accepted_baseline_major: String::new(),
        accepted_baseline_minor: String::new(),
        candidate_version: String::new(),
        candidate_binary_path: String::new(),
        ideas_file: ideas_file_str,
        ideas_file_pending_count: ideas_pending_count,
        ideas_format: if resolved_ideas_path.is_some() {
            "markdown_checklist".to_string()
        } else {
            String::new()
        },
        stockfish_binary: String::new(),
        max_iterations: max_iterations.to_string(),
        max_infra_failures: max_infra_failures.to_string(),
        session_id: session_id.clone(),
        session_dir: session_dir.to_string_lossy().to_string(),
        summary_file: summary_path.to_string_lossy().to_string(),
    };
    save_session_metadata(&session_env_path, &meta)?;
    write_placeholder_summary(&summary_path, &session_id)?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();
    ctrlc::set_handler(move || {
        warn!("received interrupt signal; will stop after current iteration");
        stop_flag_clone.store(true, Ordering::SeqCst);
    })
    .context("setting SIGINT handler")?;

    let cfg = SessionConfig {
        repo_root,
        session_dir: session_dir.clone(),
        session_env_path: session_env_path.clone(),
        summary_path: summary_path.clone(),
        phase_timeout_secs,
        verbose,
        max_iterations,
        max_infra_failures,
        stop_flag,
    };

    println!("=== Evolution Session ===");
    println!("  Session ID:       {}", session_id);
    println!("  Baseline:         {}", baseline_version);
    println!("  Max iterations:   {}", max_iterations);
    println!("  Max infra fails:  {}", max_infra_failures);
    println!("  Phase timeout:    {}s", phase_timeout_secs);
    println!("  Verbose:          {}", verbose);
    println!("  Session dir:      {}", session_dir.display());
    println!("========================");

    run_loop(&cfg, meta, 1)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Resume flow
// ---------------------------------------------------------------------------

fn run_resume(
    session_path: PathBuf,
    from: Option<String>,
    phase_timeout_secs: u64,
    verbose: bool,
) -> Result<()> {
    let session_env_path = session_path.join("session.env");
    let mut meta = load_session_metadata(&session_env_path)
        .with_context(|| format!("reading session.env from {}", session_path.display()))?;

    // Find latest iteration
    let iterations_dir = session_path.join("iterations");
    let latest_n = if iterations_dir.exists() {
        std::fs::read_dir(&iterations_dir)
            .context("reading iterations directory")?
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().to_string_lossy().parse::<u32>().ok())
            .max()
            .unwrap_or(0)
    } else {
        0
    };

    if latest_n == 0 {
        anyhow::bail!("no iterations found in {}", iterations_dir.display());
    }

    let repo_root = std::env::current_dir()?;
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();
    ctrlc::set_handler(move || {
        warn!("received interrupt signal");
        stop_flag_clone.store(true, Ordering::SeqCst);
    })
    .context("setting SIGINT handler")?;

    let summary_path = session_path.join("summary.md");
    let cfg = SessionConfig {
        repo_root,
        session_dir: session_path.clone(),
        session_env_path,
        summary_path,
        phase_timeout_secs,
        verbose,
        max_iterations: meta.max_iterations.parse().unwrap_or(10),
        max_infra_failures: meta.max_infra_failures.parse().unwrap_or(3),
        stop_flag,
    };

    let _from_phase = from.as_deref().unwrap_or("propose");

    info!(iteration = latest_n, "resuming session");
    let result = run_iteration(latest_n, &cfg, &mut meta)?;
    info!(iteration = latest_n, outcome = %result.outcome, "resumed iteration complete");

    write_session_summary(
        &cfg.summary_path,
        &cfg.session_dir,
        &meta.session_id,
        &meta.baseline_version,
        cfg.max_iterations,
        latest_n,
        "resumed_and_completed",
        "",
        &meta.accepted_baseline_version,
        &meta.accepted_baseline_path,
    )?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Main iteration loop
// ---------------------------------------------------------------------------

fn run_loop(cfg: &SessionConfig, mut meta: SessionMetadata, start_n: u32) -> Result<()> {
    let mut infra_failure_count = 0u32;
    let mut completed = 0u32;
    let mut stop_reason = "max_iterations_reached".to_string();
    let mut stop_details = String::new();

    for n in start_n..=cfg.max_iterations {
        if cfg.stop_flag.load(Ordering::SeqCst) {
            stop_reason = "interrupted".to_string();
            stop_details = "SIGINT received".to_string();
            break;
        }
        if infra_failure_count >= cfg.max_infra_failures {
            stop_reason = "max_infra_failures_reached".to_string();
            stop_details = format!("{} consecutive infra failures", infra_failure_count);
            break;
        }

        println!("[iteration {}/{}] starting", n, cfg.max_iterations);

        let result = run_iteration(n, cfg, &mut meta)?;
        completed += 1;

        println!("[iteration {}/{}] outcome: {}", n, cfg.max_iterations, result.outcome);

        // Check for session-stopping conditions from the iteration
        if let Some((reason, details)) = result.stop_session {
            stop_reason = reason;
            stop_details = details;
            break;
        }

        if result.infra_failure {
            infra_failure_count += 1;
        } else {
            infra_failure_count = 0;
        }
    }

    write_session_summary(
        &cfg.summary_path,
        &cfg.session_dir,
        &meta.session_id,
        &meta.baseline_version,
        cfg.max_iterations,
        completed,
        &stop_reason,
        &stop_details,
        &meta.accepted_baseline_version,
        &meta.accepted_baseline_path,
    )?;

    println!("Session complete. Stop reason: {}", stop_reason);
    Ok(())
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(true)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start {
            baseline_version,
            ideas_file,
            output_dir,
            max_iterations,
            max_infra_failures,
            phase_timeout_secs,
            verbose,
        } => run_start(
            baseline_version,
            ideas_file,
            output_dir,
            max_iterations,
            max_infra_failures,
            phase_timeout_secs,
            verbose,
        ),
        Commands::Resume {
            session,
            from,
            phase_timeout_secs,
            verbose,
        } => run_resume(session, from, phase_timeout_secs, verbose),
    }
}
