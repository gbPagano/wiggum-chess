use anyhow::{Context, Result};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PhaseConfig {
    pub skill_name: String,
    pub candidate_workspace: PathBuf,
    pub iteration_dir: PathBuf,
    pub iteration_state_path: PathBuf,
    pub session_dir: PathBuf,
    pub repo_root: PathBuf,
    pub session_metadata_path: PathBuf,
    pub phase_timeout_secs: u64,
    pub verbose: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseOutcome {
    Success,
    Failed(i32),
    Timeout,
}

fn claude_bin() -> String {
    std::env::var("CLAUDE_BIN").unwrap_or_else(|_| "openclaude".to_string())
}

fn build_prompt(config: &PhaseConfig) -> String {
    format!(
        "/{skill}\n\n\
        Repository root: {repo_root}\n\
        Session directory: {session_dir}\n\
        Iteration directory: {iter_dir}\n\
        Iteration state file: {iter_state}\n\
        Session metadata file: {session_meta}\n\
        Worker guidance path: {guidance}\n\
        \n\
        You are running the `{skill}` phase of the evolution loop. Read the iteration state \
        from the iteration state file and follow the guidance in the worker guidance path.\n",
        skill = config.skill_name,
        repo_root = config.repo_root.display(),
        session_dir = config.session_dir.display(),
        iter_dir = config.iteration_dir.display(),
        iter_state = config.iteration_state_path.display(),
        session_meta = config.session_metadata_path.display(),
        guidance = config.session_dir.join("worker-guidance.md").display(),
    )
}

pub fn run_phase(config: &PhaseConfig) -> Result<PhaseOutcome> {
    let log_dir = config.iteration_dir.join("phase-logs");
    fs::create_dir_all(&log_dir).context("creating phase-logs directory")?;
    let log_path = log_dir.join(format!("{}.log", config.skill_name));

    let prompt = build_prompt(config);
    let claude = claude_bin();

    let mut cmd = Command::new(&claude);
    cmd.args([
        "--dangerously-skip-permissions",
        "--add-dir",
        config.session_dir.to_str().unwrap_or(""),
        "--add-dir",
        config.repo_root.to_str().unwrap_or(""),
        "--print",
    ])
    .current_dir(&config.candidate_workspace)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawning {} for phase {}", claude, config.skill_name))?;

    // Write prompt to stdin
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(prompt.as_bytes());
    }

    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");

    let log_path_clone = log_path.clone();
    let verbose = config.verbose;

    // Reader thread: captures stdout+stderr, tees to log and optionally to terminal
    let reader_thread = thread::spawn(move || -> std::io::Result<()> {
        let mut log_file = fs::File::create(&log_path_clone)?;

        // Merge stdout and stderr into log (simple sequential read)
        let mut stdout_reader = BufReader::new(stdout);
        let mut stderr_reader = BufReader::new(stderr);

        let mut stdout_done = false;
        let mut stderr_done = false;

        loop {
            if !stdout_done {
                let mut line = String::new();
                match stdout_reader.read_line(&mut line) {
                    Ok(0) => stdout_done = true,
                    Ok(_) => {
                        log_file.write_all(line.as_bytes())?;
                        if verbose {
                            print!("[evolution-loop] {}", line);
                        }
                    }
                    Err(_) => stdout_done = true,
                }
            }
            if !stderr_done {
                let mut line = String::new();
                match stderr_reader.read_line(&mut line) {
                    Ok(0) => stderr_done = true,
                    Ok(_) => {
                        log_file.write_all(line.as_bytes())?;
                        if verbose {
                            eprint!("[evolution-loop] {}", line);
                        }
                    }
                    Err(_) => stderr_done = true,
                }
            }
            if stdout_done && stderr_done {
                break;
            }
        }
        Ok(())
    });

    // Wait for process with timeout (polling)
    let timeout = Duration::from_secs(config.phase_timeout_secs);
    let start = std::time::Instant::now();
    let outcome = loop {
        match child.try_wait().context("polling child process")? {
            Some(status) => {
                let code = status.code().unwrap_or(-1);
                break if status.success() {
                    PhaseOutcome::Success
                } else {
                    PhaseOutcome::Failed(code)
                };
            }
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    break PhaseOutcome::Timeout;
                }
                thread::sleep(Duration::from_millis(500));
            }
        }
    };

    let _ = reader_thread.join();
    Ok(outcome)
}
