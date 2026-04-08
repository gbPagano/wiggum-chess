use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::state::{
    save_iteration_state, ArtifactsState, CandidateState, CorrectnessState,
    DecisionState, IdeasState, IsolationState, IterationPhase, IterationState,
    SessionMetadata, StateMachineState, StockfishComparisonState,
};

pub struct IterationPaths {
    pub iteration_dir: PathBuf,
    pub iteration_json: PathBuf,
    pub hypothesis_md: PathBuf,
    pub implementation_md: PathBuf,
    pub correctness_results_md: PathBuf,
    pub benchmark_md: PathBuf,
    pub stockfish_comparison_md: PathBuf,
    pub decision_md: PathBuf,
    pub phase_logs_dir: PathBuf,
}

fn write_placeholder(path: &Path, title: &str) -> Result<()> {
    let mut f = fs::File::create(path)
        .with_context(|| format!("creating {}", path.display()))?;
    writeln!(f, "# {}", title)?;
    writeln!(f)?;
    writeln!(f, "_Pending phase execution._")?;
    Ok(())
}

pub fn create_iteration_artifacts(
    session_dir: &Path,
    iteration: u32,
    session_meta: &SessionMetadata,
    candidate_workspace: &Path,
    candidate_branch: &str,
    candidate_setup_status: &str,
    candidate_setup_error: &str,
) -> Result<IterationPaths> {
    let iter_dir = session_dir.join("iterations").join(iteration.to_string());
    let correctness_dir = iter_dir.join("correctness");
    let stockfish_dir = iter_dir.join("stockfish-comparison");
    let phase_logs_dir = iter_dir.join("phase-logs");

    for dir in &[&iter_dir, &correctness_dir, &stockfish_dir, &phase_logs_dir] {
        fs::create_dir_all(dir)
            .with_context(|| format!("creating directory {}", dir.display()))?;
    }

    let hypothesis_md = iter_dir.join("hypothesis.md");
    let implementation_md = iter_dir.join("implementation.md");
    let correctness_results_md = correctness_dir.join("results.md");
    let benchmark_md = iter_dir.join("benchmark.md");
    let stockfish_comparison_md = stockfish_dir.join("results.md");
    let decision_md = iter_dir.join("decision.md");
    let iteration_json = iter_dir.join("iteration.json");

    write_placeholder(&hypothesis_md, "Hypothesis")?;
    write_placeholder(&implementation_md, "Implementation")?;
    write_placeholder(&correctness_results_md, "Correctness Results")?;
    write_placeholder(&benchmark_md, "Benchmark Results")?;
    write_placeholder(&stockfish_comparison_md, "Stockfish Comparison")?;
    write_placeholder(&decision_md, "Decision")?;

    let failed_setup = candidate_setup_status == "failed";

    let mut phase_logs: HashMap<String, String> = HashMap::new();
    for phase in &["propose", "implement", "benchmark", "decide"] {
        phase_logs.insert(
            phase.to_string(),
            phase_logs_dir.join(format!("{}.log", phase)).to_string_lossy().to_string(),
        );
    }

    let (state, correctness_status, decision) = if failed_setup {
        (
            IterationPhase::Failed,
            CorrectnessState {
                status: "skipped".to_string(),
                passed: false,
                benchmark_eligible: false,
                checks: vec![],
            },
            Some(DecisionState {
                outcome: "failed".to_string(),
                evidence: Some(candidate_setup_error.to_string()),
                promotion: None,
            }),
        )
    } else {
        (
            IterationPhase::Initialized,
            CorrectnessState {
                status: "pending".to_string(),
                passed: false,
                benchmark_eligible: false,
                checks: vec![],
            },
            None,
        )
    };

    let baseline_report = {
        let report = Path::new(&session_meta.active_baseline_path).join("report.md");
        report.exists()
    };

    let iter_state = IterationState {
        iteration,
        baseline_version: session_meta.active_baseline_version.clone(),
        baseline_path: session_meta.active_baseline_path.clone(),
        baseline_binary: session_meta.active_baseline_binary.clone(),
        ideas: IdeasState {
            proposal_source: None,
            selected_idea: None,
            selected_idea_marked_used: Some(false),
        },
        candidate: CandidateState {
            version: if failed_setup { None } else { Some(session_meta.candidate_version.clone()) },
            binary_path: if failed_setup { None } else { Some(session_meta.candidate_binary_path.clone()) },
            workspace: if failed_setup { None } else { Some(candidate_workspace.to_string_lossy().to_string()) },
            branch: if failed_setup { None } else { Some(candidate_branch.to_string()) },
            setup_status: Some(candidate_setup_status.to_string()),
            setup_error: if candidate_setup_error.is_empty() { None } else { Some(candidate_setup_error.to_string()) },
        },
        state: state.clone(),
        isolation: IsolationState {
            worktree: if failed_setup { None } else { Some(candidate_workspace.to_string_lossy().to_string()) },
            branch: if failed_setup { None } else { Some(candidate_branch.to_string()) },
        },
        correctness: correctness_status,
        stockfish_comparison: StockfishComparisonState {
            baseline_report_available: Some(baseline_report),
            recommendation_changed: None,
            limitation: None,
            positive_signal: None,
        },
        state_machine: StateMachineState {
            current: state,
            transitions: vec![],
        },
        artifacts: ArtifactsState {
            hypothesis: Some(hypothesis_md.to_string_lossy().to_string()),
            implementation: Some(implementation_md.to_string_lossy().to_string()),
            correctness_results: Some(correctness_results_md.to_string_lossy().to_string()),
            benchmark: Some(benchmark_md.to_string_lossy().to_string()),
            stockfish_comparison: Some(stockfish_comparison_md.to_string_lossy().to_string()),
            decision: Some(decision_md.to_string_lossy().to_string()),
            phase_logs: Some(phase_logs),
        },
        decision,
    };

    save_iteration_state(&iteration_json, &iter_state)?;

    Ok(IterationPaths {
        iteration_dir: iter_dir,
        iteration_json,
        hypothesis_md,
        implementation_md,
        correctness_results_md,
        benchmark_md,
        stockfish_comparison_md,
        decision_md,
        phase_logs_dir,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn dummy_meta(session_dir: &Path) -> SessionMetadata {
        SessionMetadata {
            baseline_version: "v1.0".to_string(),
            active_baseline_version: "v1.0".to_string(),
            active_baseline_path: session_dir.to_string_lossy().to_string(),
            active_baseline_binary: session_dir
                .join("wiggum-engine")
                .to_string_lossy()
                .to_string(),
            accepted_baseline_version: "v1.0".to_string(),
            accepted_baseline_path: session_dir.to_string_lossy().to_string(),
            accepted_baseline_binary: session_dir
                .join("wiggum-engine")
                .to_string_lossy()
                .to_string(),
            accepted_baseline_major: "1".to_string(),
            accepted_baseline_minor: "0".to_string(),
            candidate_version: "v1.1".to_string(),
            candidate_binary_path: "/tmp/candidate/wiggum-engine".to_string(),
            ideas_file: String::new(),
            ideas_file_pending_count: "0".to_string(),
            ideas_format: String::new(),
            stockfish_binary: String::new(),
            max_iterations: "10".to_string(),
            max_infra_failures: "3".to_string(),
            session_id: "test-session".to_string(),
            session_dir: session_dir.to_string_lossy().to_string(),
            summary_file: session_dir.join("summary.md").to_string_lossy().to_string(),
        }
    }

    #[test]
    fn creates_all_directories_and_placeholder_files() {
        let tmp = TempDir::new().unwrap();
        let session_dir = tmp.path();
        let meta = dummy_meta(session_dir);
        let candidate_workspace = Path::new("/tmp/candidate");

        let paths = create_iteration_artifacts(
            session_dir,
            1,
            &meta,
            candidate_workspace,
            "wiggum-evolution/test-session/iteration-1",
            "ok",
            "",
        )
        .unwrap();

        assert!(paths.iteration_dir.exists());
        assert!(paths.iteration_dir.join("correctness").exists());
        assert!(paths.iteration_dir.join("stockfish-comparison").exists());
        assert!(paths.phase_logs_dir.exists());
        assert!(paths.hypothesis_md.exists());
        assert!(paths.implementation_md.exists());
        assert!(paths.correctness_results_md.exists());
        assert!(paths.benchmark_md.exists());
        assert!(paths.stockfish_comparison_md.exists());
        assert!(paths.decision_md.exists());
        assert!(paths.iteration_json.exists());
    }

    #[test]
    fn successful_setup_writes_initialized_state() {
        let tmp = TempDir::new().unwrap();
        let session_dir = tmp.path();
        let meta = dummy_meta(session_dir);
        let candidate_workspace = Path::new("/tmp/candidate");

        let paths = create_iteration_artifacts(
            session_dir,
            2,
            &meta,
            candidate_workspace,
            "wiggum-evolution/test-session/iteration-2",
            "ok",
            "",
        )
        .unwrap();

        let iter_state = crate::state::load_iteration_state(&paths.iteration_json).unwrap();
        assert_eq!(iter_state.state, IterationPhase::Initialized);
        assert_eq!(iter_state.correctness.status, "pending");
        assert!(iter_state.decision.is_none());
        assert_eq!(iter_state.iteration, 2);
    }

    #[test]
    fn failed_setup_writes_failed_state_and_decision() {
        let tmp = TempDir::new().unwrap();
        let session_dir = tmp.path();
        let meta = dummy_meta(session_dir);
        let candidate_workspace = Path::new("/tmp/candidate");

        let paths = create_iteration_artifacts(
            session_dir,
            3,
            &meta,
            candidate_workspace,
            "",
            "failed",
            "git worktree add failed: ref not found",
        )
        .unwrap();

        let iter_state = crate::state::load_iteration_state(&paths.iteration_json).unwrap();
        assert_eq!(iter_state.state, IterationPhase::Failed);
        assert_eq!(iter_state.correctness.status, "skipped");
        assert!(!iter_state.correctness.benchmark_eligible);
        let decision = iter_state.decision.unwrap();
        assert_eq!(decision.outcome, "failed");
        assert!(decision.evidence.as_deref().unwrap_or("").contains("git worktree"));
    }

    #[test]
    fn phase_log_paths_recorded_in_iteration_json() {
        let tmp = TempDir::new().unwrap();
        let session_dir = tmp.path();
        let meta = dummy_meta(session_dir);
        let candidate_workspace = Path::new("/tmp/candidate");

        let paths = create_iteration_artifacts(
            session_dir,
            4,
            &meta,
            candidate_workspace,
            "wiggum-evolution/test-session/iteration-4",
            "ok",
            "",
        )
        .unwrap();

        let iter_state = crate::state::load_iteration_state(&paths.iteration_json).unwrap();
        let phase_logs = iter_state.artifacts.phase_logs.unwrap();
        assert!(phase_logs.contains_key("propose"));
        assert!(phase_logs.contains_key("implement"));
        assert!(phase_logs.contains_key("benchmark"));
        assert!(phase_logs.contains_key("decide"));
        assert!(phase_logs["propose"].ends_with("propose.log"));
    }
}
