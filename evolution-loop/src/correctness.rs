use anyhow::{Context, Result};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::state::{
    load_iteration_state, save_iteration_state, CheckResult, CorrectnessState, IterationPhase,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectnessOutcome {
    Passed,
    Failed(Vec<CheckResult>),
}

struct TimedResult {
    status: String,
    stdout: String,
    stderr: String,
    timed_out: bool,
}

fn run_with_timeout(program: &str, args: &[&str], cwd: &Path, timeout_secs: u64) -> TimedResult {
    let mut child = match Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return TimedResult {
                status: "failed".to_string(),
                stdout: String::new(),
                stderr: e.to_string(),
                timed_out: false,
            };
        }
    };

    let timeout = Duration::from_secs(timeout_secs);
    let result = child.wait_timeout(timeout);

    match result {
        Ok(Some(status)) => {
            let mut stdout_bytes = vec![];
            let mut stderr_bytes = vec![];
            if let Some(mut s) = child.stdout.take() {
                let _ = s.read_to_end(&mut stdout_bytes);
            }
            if let Some(mut s) = child.stderr.take() {
                let _ = s.read_to_end(&mut stderr_bytes);
            }
            TimedResult {
                status: if status.success() { "passed" } else { "failed" }.to_string(),
                stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
                stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
                timed_out: false,
            }
        }
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
            TimedResult {
                status: "failed".to_string(),
                stdout: String::new(),
                stderr: format!("timed out after {} seconds", timeout_secs),
                timed_out: true,
            }
        }
        Err(e) => TimedResult {
            status: "failed".to_string(),
            stdout: String::new(),
            stderr: e.to_string(),
            timed_out: false,
        },
    }
}

// We need wait_timeout — use a simple polling approach without extra deps.
trait WaitTimeout {
    fn wait_timeout(&mut self, duration: Duration) -> Result<Option<std::process::ExitStatus>>;
}

impl WaitTimeout for std::process::Child {
    fn wait_timeout(&mut self, duration: Duration) -> Result<Option<std::process::ExitStatus>> {
        let start = std::time::Instant::now();
        loop {
            match self.try_wait()? {
                Some(status) => return Ok(Some(status)),
                None => {
                    if start.elapsed() >= duration {
                        return Ok(None);
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }
}

pub fn run_correctness_gate(
    candidate_dir: &Path,
    iteration_state_path: &Path,
    correctness_results_path: &Path,
    timeout_secs: u64,
) -> Result<CorrectnessOutcome> {
    let checks_spec: &[(&str, &[&str], &str)] = &[
        ("cargo build", &["build", "--workspace"], "cargo build --workspace"),
        (
            "cargo test",
            &[
                "test",
                "--workspace",
                "--",
                "--skip",
                "gen_files::magics::name",
            ],
            "cargo test --workspace -- --skip gen_files::magics::name",
        ),
    ];

    let mut check_results: Vec<CheckResult> = Vec::new();

    for (name, args, display) in checks_spec {
        let timed = run_with_timeout("cargo", args, candidate_dir, timeout_secs);
        let reason = if timed.timed_out {
            Some("timeout".to_string())
        } else if timed.status == "failed" {
            let detail = if !timed.stderr.trim().is_empty() {
                timed.stderr.trim().to_string()
            } else if !timed.stdout.trim().is_empty() {
                timed.stdout.trim().to_string()
            } else {
                format!("command failed: {}", display)
            };
            Some(detail.chars().take(500).collect())
        } else {
            None
        };
        check_results.push(CheckResult {
            name: name.to_string(),
            status: timed.status.clone(),
            reason,
        });
        if timed.status != "passed" {
            break;
        }
    }

    let all_passed = check_results.iter().all(|c| c.status == "passed");

    // Write correctness/results.md
    if let Some(parent) = correctness_results_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    {
        let mut f = fs::File::create(correctness_results_path)
            .with_context(|| format!("creating {}", correctness_results_path.display()))?;
        writeln!(f, "# Correctness Gate Results\n")?;
        writeln!(f, "Status: {}\n", if all_passed { "passed" } else { "failed" })?;
        for check in &check_results {
            writeln!(f, "## {}", check.name)?;
            writeln!(f, "- Status: {}", check.status)?;
            if let Some(r) = &check.reason {
                writeln!(f, "- Reason: {}", r)?;
            }
            writeln!(f)?;
        }
    }

    // Update iteration.json
    let mut iter_state = load_iteration_state(iteration_state_path)?;
    iter_state.correctness = CorrectnessState {
        status: if all_passed { "passed" } else { "failed" }.to_string(),
        passed: all_passed,
        benchmark_eligible: all_passed,
        checks: check_results.clone(),
    };
    iter_state.state = if all_passed {
        IterationPhase::Implemented
    } else {
        IterationPhase::Failed
    };
    save_iteration_state(iteration_state_path, &iter_state)?;

    if all_passed {
        Ok(CorrectnessOutcome::Passed)
    } else {
        Ok(CorrectnessOutcome::Failed(check_results))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{
        ArtifactsState, CandidateState, IdeasState, IsolationState, IterationState,
        StateMachineState, StockfishComparisonState,
    };
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn sample_iteration_state() -> IterationState {
        IterationState {
            iteration: 1,
            baseline_version: "v0.1".to_string(),
            baseline_path: "/baseline".to_string(),
            baseline_binary: "/baseline/wiggum-engine".to_string(),
            ideas: IdeasState {
                proposal_source: Some("self_proposed".to_string()),
                selected_idea: Some("Improve move ordering".to_string()),
                selected_idea_marked_used: Some(false),
            },
            candidate: CandidateState {
                version: Some("v0.2".to_string()),
                binary_path: Some("/candidate/wiggum-engine".to_string()),
                workspace: Some("/candidate".to_string()),
                branch: Some("wiggum-evolution/session/iteration-1".to_string()),
                setup_status: Some("ok".to_string()),
                setup_error: None,
            },
            state: IterationPhase::Validating,
            isolation: IsolationState {
                worktree: Some("/candidate".to_string()),
                branch: Some("wiggum-evolution/session/iteration-1".to_string()),
            },
            correctness: CorrectnessState {
                status: "pending".to_string(),
                passed: false,
                benchmark_eligible: false,
                checks: vec![],
            },
            stockfish_comparison: StockfishComparisonState {
                baseline_report_available: Some(false),
                recommendation_changed: None,
                limitation: None,
                positive_signal: None,
            },
            state_machine: StateMachineState {
                current: IterationPhase::Validating,
                transitions: vec![],
            },
            artifacts: ArtifactsState {
                hypothesis: None,
                implementation: None,
                correctness_results: None,
                benchmark: None,
                stockfish_comparison: None,
                decision: None,
                phase_logs: Some(HashMap::new()),
            },
            decision: None,
        }
    }

    fn write_minimal_workspace(root: &Path, test_body: &str) {
        write_file(
            &root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"engine\"]\nresolver = \"2\"\n",
        );
        write_file(
            &root.join("engine/Cargo.toml"),
            "[package]\nname = \"engine\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        );
        write_file(&root.join("engine/src/lib.rs"), test_body);
    }

    #[test]
    fn run_correctness_gate_records_success() {
        let temp = tempdir().unwrap();
        write_minimal_workspace(
            temp.path(),
            "#[test]\nfn smoke() { assert_eq!(2 + 2, 4); }\n",
        );

        let iteration_state_path = temp.path().join("iteration.json");
        let correctness_results_path = temp.path().join("correctness/results.md");
        save_iteration_state(&iteration_state_path, &sample_iteration_state()).unwrap();

        let outcome = run_correctness_gate(
            temp.path(),
            &iteration_state_path,
            &correctness_results_path,
            30,
        )
        .unwrap();

        assert_eq!(outcome, CorrectnessOutcome::Passed);

        let iteration = load_iteration_state(&iteration_state_path).unwrap();
        assert_eq!(iteration.correctness.status, "passed");
        assert!(iteration.correctness.passed);
        assert!(iteration.correctness.benchmark_eligible);
        assert_eq!(iteration.correctness.checks.len(), 2);
        assert_eq!(iteration.state, IterationPhase::Implemented);

        let results = fs::read_to_string(&correctness_results_path).unwrap();
        assert!(results.contains("Status: passed"));
        assert!(results.contains("## cargo build"));
        assert!(results.contains("## cargo test"));
    }

    #[test]
    fn run_correctness_gate_records_failure_and_stops_before_tests() {
        let temp = tempdir().unwrap();
        write_minimal_workspace(temp.path(), "this will not compile\n");

        let iteration_state_path = temp.path().join("iteration.json");
        let correctness_results_path = temp.path().join("correctness/results.md");
        save_iteration_state(&iteration_state_path, &sample_iteration_state()).unwrap();

        let outcome = run_correctness_gate(
            temp.path(),
            &iteration_state_path,
            &correctness_results_path,
            30,
        )
        .unwrap();

        match outcome {
            CorrectnessOutcome::Passed => panic!("expected failure"),
            CorrectnessOutcome::Failed(checks) => {
                assert_eq!(checks.len(), 1);
                assert_eq!(checks[0].name, "cargo build");
                assert_eq!(checks[0].status, "failed");
                assert!(checks[0].reason.as_ref().is_some_and(|reason| !reason.is_empty()));
            }
        }

        let iteration = load_iteration_state(&iteration_state_path).unwrap();
        assert_eq!(iteration.correctness.status, "failed");
        assert!(!iteration.correctness.passed);
        assert!(!iteration.correctness.benchmark_eligible);
        assert_eq!(iteration.correctness.checks.len(), 1);
        assert_eq!(iteration.state, IterationPhase::Failed);

        let results = fs::read_to_string(&correctness_results_path).unwrap();
        assert!(results.contains("Status: failed"));
        assert!(results.contains("## cargo build"));
        assert!(!results.contains("## cargo test"));
    }
}
