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
    load_iteration_state, load_session_metadata, save_session_metadata, IterationPhase,
    SessionMetadata,
};
use summary::{write_placeholder_summary, write_session_summary};
use versioning::{
    apply_candidate_manifest_versions, candidate_version_for_source, cargo_semver_from_tag,
    parse_version_tag, ProposalSource,
};
use worktree::{
    candidate_branch_name, commit_candidate_workspace, create_candidate_branch,
    create_candidate_workspace, remove_candidate_workspace,
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
// Iteration runner
// ---------------------------------------------------------------------------

struct IterationResult {
    outcome: String, // accepted / rejected / inconclusive / failed
    infra_failure: bool,
}

fn run_iteration(
    n: u32,
    cfg: &SessionConfig,
    meta: &mut SessionMetadata,
) -> Result<IterationResult> {
    info!(iteration = n, "starting iteration");

    // Compute candidate version from ideas file presence
    let candidate_version = if meta.candidate_version.is_empty() {
        // Derive a provisional version; will be refined after propose phase
        candidate_version_for_source(&meta.active_baseline_version, ProposalSource::SelfProposed)?
    } else {
        meta.candidate_version.clone()
    };

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

    meta.candidate_version = candidate_version;
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
        remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
        return Ok(IterationResult {
            outcome: "failed".to_string(),
            infra_failure: true,
        });
    }

    // Bump Cargo versions in candidate workspace
    let semver = cargo_semver_from_tag(&meta.candidate_version)?;
    let _ = apply_candidate_manifest_versions(&candidate_dir, &semver);

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
            warn!(iteration = n, phase = "propose", "phase timed out");
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true });
        }
        PhaseOutcome::Failed(code) => {
            warn!(iteration = n, phase = "propose", exit_code = code, "phase failed");
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true });
        }
        PhaseOutcome::Success => {
            info!(iteration = n, phase = "propose", outcome = "success", "phase complete");
        }
    }

    // Check for no-hypothesis stop condition
    let iter_state = load_iteration_state(&paths.iteration_json)?;
    if iter_state.ideas.proposal_source.is_none() {
        info!(iteration = n, "propose phase produced no hypothesis; stopping session");
        remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
        return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: false });
    }

    // --- implement ---
    info!(iteration = n, phase = "implement", "running phase");
    match run_phase(&make_phase_config("evolution-implement"))? {
        PhaseOutcome::Timeout => {
            warn!(iteration = n, phase = "implement", "phase timed out");
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true });
        }
        PhaseOutcome::Failed(code) => {
            warn!(iteration = n, phase = "implement", exit_code = code, "phase failed");
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true });
        }
        PhaseOutcome::Success => {
            info!(iteration = n, phase = "implement", outcome = "success", "phase complete");
        }
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
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: false });
        }
    }

    // --- benchmark ---
    info!(iteration = n, phase = "benchmark", "running phase");
    match run_phase(&make_phase_config("evolution-benchmark"))? {
        PhaseOutcome::Timeout => {
            warn!(iteration = n, phase = "benchmark", timeout_secs = cfg.phase_timeout_secs, "phase timed out");
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true });
        }
        PhaseOutcome::Failed(code) => {
            warn!(iteration = n, phase = "benchmark", exit_code = code, "phase failed");
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true });
        }
        PhaseOutcome::Success => {
            info!(iteration = n, phase = "benchmark", outcome = "success", "phase complete");
        }
    }

    // --- decide ---
    info!(iteration = n, phase = "decide", "running phase");
    match run_phase(&make_phase_config("evolution-decide"))? {
        PhaseOutcome::Timeout => {
            warn!(iteration = n, phase = "decide", timeout_secs = cfg.phase_timeout_secs, "phase timed out");
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true });
        }
        PhaseOutcome::Failed(code) => {
            warn!(iteration = n, phase = "decide", exit_code = code, "phase failed");
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            return Ok(IterationResult { outcome: "failed".to_string(), infra_failure: true });
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
    let proposal_source = final_state
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
                    let mark_result = mark_idea_used(&p, &selected_idea, &proposal_source)
                        .unwrap_or(MarkResult::Skipped);
                    let _ = update_session_after_mark(meta, &p, &mark_result);
                }
            }
            // Promote
            if let Err(e) = promote_candidate(
                &candidate_dir,
                &paths.iteration_json,
                meta,
                &cfg.repo_root,
            ) {
                warn!(iteration = n, error = %e, "promotion failed");
            }
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            save_session_metadata(&cfg.session_env_path, meta)?;
            false
        }
        "rejected" | "inconclusive" => {
            // Mark idea used
            let mut infra = false;
            if let Ok(ideas_path) = resolve_ideas_file(&meta.ideas_file, &cfg.repo_root) {
                if let Some(p) = ideas_path {
                    let mark_result = mark_idea_used(&p, &selected_idea, &proposal_source)
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
            remove_candidate_workspace(&cfg.repo_root, &candidate_dir);
            true
        }
    };

    info!(iteration = n, outcome = %outcome, "iteration complete");
    Ok(IterationResult { outcome, infra_failure })
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
        ideas_format: if resolved_ideas_path.is_some() { "markdown_checklist".to_string() } else { String::new() },
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
            .filter_map(|e| {
                e.file_name()
                    .to_string_lossy()
                    .parse::<u32>()
                    .ok()
            })
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
