use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
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
    exit_code: Option<i32>,
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
                exit_code: None,
                stderr: e.to_string(),
                timed_out: false,
            };
        }
    };

    let timeout = Duration::from_secs(timeout_secs);
    let result = child.wait_timeout(timeout);

    match result {
        Ok(Some(status)) => {
            let exit_code = status.code();
            let mut stderr_bytes = vec![];
            if let Some(mut s) = child.stderr.take() {
                use std::io::Read;
                let _ = s.read_to_end(&mut stderr_bytes);
            }
            TimedResult {
                status: if status.success() { "passed" } else { "failed" }.to_string(),
                exit_code,
                stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
                timed_out: false,
            }
        }
        Ok(None) => {
            // Timeout — kill the process
            let _ = child.kill();
            TimedResult {
                status: "failed".to_string(),
                exit_code: None,
                stderr: format!("timed out after {} seconds", timeout_secs),
                timed_out: true,
            }
        }
        Err(e) => TimedResult {
            status: "failed".to_string(),
            exit_code: None,
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

    for (name, args, _display) in checks_spec {
        let timed = run_with_timeout("cargo", args, candidate_dir, timeout_secs);
        let reason = if timed.timed_out {
            Some("timeout".to_string())
        } else if timed.status == "failed" {
            Some(timed.stderr.chars().take(500).collect())
        } else {
            None
        };
        check_results.push(CheckResult {
            name: name.to_string(),
            status: timed.status.clone(),
            reason,
        });
        if timed.status != "passed" {
            break; // Stop on first failure
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
    if all_passed {
        iter_state.state = IterationPhase::Implemented;
    }
    save_iteration_state(iteration_state_path, &iter_state)?;

    if all_passed {
        Ok(CorrectnessOutcome::Passed)
    } else {
        Ok(CorrectnessOutcome::Failed(check_results))
    }
}
