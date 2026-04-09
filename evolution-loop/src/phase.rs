use anyhow::{anyhow, Context, Result};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tracing::debug;

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

const PHASE_GUIDANCE: &str = r#"Read and update the iteration artifacts at the paths recorded in iteration.json.
If iteration.json or session.env points to an ideas file, treat it as an optional propose-phase input. The file format is Markdown checklist entries like `- [ ] idea text`, and only unchecked entries are pending ideas.
If the ideas file field is empty, missing, or has zero pending checklist entries, behave exactly like the default self-propose flow.
During /evolution-propose, always set `ideas.proposalSource` in iteration.json to either `user_ideas_file` or `self_proposed`. If you selected a checklist idea, also set `ideas.selectedIdea` to the exact checklist text and state the source clearly in hypothesis.md. If you self-propose, clear `ideas.selectedIdea` to an empty string and still state the source in hypothesis.md.
Implementation, benchmark, and decision phases must treat `iteration.json` as the source of truth for the proposal source metadata instead of inferring it from hypothesis text.
Benchmark and decision phases must resolve the stored baseline engine from `iteration.json.baselinePath` and `iteration.json.baselineBinary` (or `session.env` `accepted_baseline_path` / `accepted_baseline_binary`) instead of relying on git refs.
If the direct candidate-vs-baseline benchmark result is inconclusive, the benchmark and/or decision flow may use `iteration.json.stockfishComparison` plus the linked artifact path in `artifacts.stockfishComparison` to run or record an additional candidate-vs-Stockfish comparison. Reuse the stored baseline Stockfish report at `iteration.json.stockfishComparison.baselineReport` when available, write the candidate follow-up result in `benchmark.md` or `stockfish-comparison/results.md`, set `stockfishComparison.changedRecommendation`, and make `decision.md` state explicitly whether the Stockfish comparison changed the recommendation or could not be completed.
If both the direct benchmark and the Stockfish comparison remain inconclusive, the decision phase must explicitly evaluate whether any positive signal remains. Record that judgment in `iteration.json.stockfishComparison.positiveSignal` with `evaluated`, `present`, `summary`, and `evidence`. This positive-signal path is the only exception that may allow an `accepted` outcome when `benchmark.sufficientForPromotion` is still false: only allow promotion from otherwise inconclusive evidence when `positiveSignal.present` is true, write the exact supporting evidence into `decision.md` and `iteration.json.decision.evidence`, and explain why promotion was allowed despite inconclusive match evidence; otherwise leave the candidate unpromoted.
If you cannot complete the phase, record the failure in the appropriate iteration artifact and iteration.json.
If no valid next hypothesis exists during /evolution-propose, record a stop signal in iteration.json using hypothesis.status = "no_hypothesis" and explain it in hypothesis.md."#;

fn claude_bin() -> String {
    std::env::var("CLAUDE_BIN").unwrap_or_else(|_| "openclaude".to_string())
}

fn logical_phase_name(skill_name: &str) -> &str {
    skill_name.strip_prefix("evolution-").unwrap_or(skill_name)
}

fn worker_guidance_path(repo_root: &Path) -> PathBuf {
    repo_root
        .join(".claude")
        .join("evolution")
        .join("CLAUDE.md")
}

fn build_prompt(config: &PhaseConfig) -> String {
    format!(
        "/{skill}\n\nRun only this iteration phase.\n\n- Repository root: {repo_root}\n- Session directory: {session_dir}\n- Iteration directory: {iteration_dir}\n- Iteration state file: {iteration_state}\n- Session metadata file: {session_metadata}\n- Worker guidance path: {worker_guidance}\n\n{phase_guidance}\n",
        skill = config.skill_name,
        repo_root = config.repo_root.display(),
        session_dir = config.session_dir.display(),
        iteration_dir = config.iteration_dir.display(),
        iteration_state = config.iteration_state_path.display(),
        session_metadata = config.session_metadata_path.display(),
        worker_guidance = worker_guidance_path(&config.repo_root).display(),
        phase_guidance = PHASE_GUIDANCE,
    )
}

fn spawn_reader<R>(
    reader: R,
    log_file: Arc<Mutex<File>>,
    verbose: bool,
) -> thread::JoinHandle<std::io::Result<()>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = reader;
        let mut buffer = [0_u8; 8192];

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            {
                let mut file = log_file.lock().expect("phase log mutex poisoned");
                file.write_all(&buffer[..bytes_read])?;
                file.flush()?;
            }

            if verbose {
                let mut stdout = std::io::stdout().lock();
                stdout.write_all(&buffer[..bytes_read])?;
                stdout.flush()?;
            }
        }

        Ok(())
    })
}

fn join_reader(handle: thread::JoinHandle<std::io::Result<()>>, stream_name: &str) -> Result<()> {
    handle
        .join()
        .map_err(|_| anyhow!("{} reader thread panicked", stream_name))?
        .with_context(|| format!("reading {} stream", stream_name))
}

pub fn run_phase(config: &PhaseConfig) -> Result<PhaseOutcome> {
    let phase_name = logical_phase_name(&config.skill_name);
    let log_dir = config.iteration_dir.join("phase-logs");
    fs::create_dir_all(&log_dir).context("creating phase-logs directory")?;
    let log_path = log_dir.join(format!("{}.log", phase_name));
    let log_file =
        Arc::new(Mutex::new(File::create(&log_path).with_context(|| {
            format!("creating phase log {}", log_path.display())
        })?));

    let prompt = build_prompt(config);
    let claude = claude_bin();

    debug!(
        skill_name = %config.skill_name,
        candidate_workspace = %config.candidate_workspace.display(),
        session_dir = %config.session_dir.display(),
        repo_root = %config.repo_root.display(),
        "running Claude phase command: {} --dangerously-skip-permissions --add-dir {} --add-dir {} --print",
        claude,
        config.session_dir.display(),
        config.repo_root.display(),
    );

    let mut child = Command::new(&claude)
        .args([
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
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawning {} for phase {}", claude, config.skill_name))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .with_context(|| format!("writing prompt for phase {}", config.skill_name))?;
    }

    let stdout = child.stdout.take().context("capturing Claude stdout")?;
    let stderr = child.stderr.take().context("capturing Claude stderr")?;

    let stdout_reader = spawn_reader(stdout, Arc::clone(&log_file), config.verbose);
    let stderr_reader = spawn_reader(stderr, Arc::clone(&log_file), config.verbose);

    let timeout = Duration::from_secs(config.phase_timeout_secs);
    let started_at = Instant::now();
    let outcome = loop {
        match child.try_wait().context("polling Claude phase process")? {
            Some(status) => {
                break if status.success() {
                    PhaseOutcome::Success
                } else {
                    PhaseOutcome::Failed(status.code().unwrap_or(-1))
                };
            }
            None if started_at.elapsed() >= timeout => {
                child
                    .kill()
                    .context("killing timed out Claude phase process")?;
                let _ = child.wait();
                break PhaseOutcome::Timeout;
            }
            None => thread::sleep(Duration::from_millis(100)),
        }
    };

    join_reader(stdout_reader, "stdout")?;
    join_reader(stderr_reader, "stderr")?;

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn make_executable(path: &Path) {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    fn sample_config(root: &Path) -> PhaseConfig {
        let session_dir = root.join("session");
        let iteration_dir = session_dir.join("iterations/1");
        let candidate_workspace = root.join("candidate");
        fs::create_dir_all(&iteration_dir).unwrap();
        fs::create_dir_all(&candidate_workspace).unwrap();

        PhaseConfig {
            skill_name: "evolution-propose".to_string(),
            candidate_workspace,
            iteration_dir: iteration_dir.clone(),
            iteration_state_path: iteration_dir.join("iteration.json"),
            session_dir,
            repo_root: root.to_path_buf(),
            session_metadata_path: root.join("session.env"),
            phase_timeout_secs: 1,
            verbose: false,
        }
    }

    #[test]
    fn build_prompt_includes_shell_guidance() {
        let temp = tempdir().unwrap();
        let config = sample_config(temp.path());
        let prompt = build_prompt(&config);

        assert!(prompt.contains("/evolution-propose"));
        assert!(prompt.contains("Run only this iteration phase."));
        assert!(prompt.contains("Worker guidance path:"));
        assert!(prompt.contains("ideas.proposalSource"));
        assert!(prompt.contains("hypothesis.status = \"no_hypothesis\""));
        assert!(prompt.contains(&config.repo_root.display().to_string()));
    }

    #[test]
    fn run_phase_writes_phase_log_and_prompt() {
        let _guard = env_lock().lock().unwrap();
        let temp = tempdir().unwrap();
        let script_path = temp.path().join("fake-claude.sh");
        let captured_prompt = temp.path().join("prompt.txt");
        fs::write(
            &script_path,
            format!(
                "#!/bin/sh\ncat > \"{}\"\nprintf 'stdout line\\n'\nprintf 'stderr line\\n' >&2\n",
                captured_prompt.display()
            ),
        )
        .unwrap();
        make_executable(&script_path);

        let previous_claude_bin = env::var_os("CLAUDE_BIN");
        env::set_var("CLAUDE_BIN", &script_path);

        let config = sample_config(temp.path());
        let outcome = run_phase(&config).unwrap();
        assert_eq!(outcome, PhaseOutcome::Success);

        let log_path = config.iteration_dir.join("phase-logs/propose.log");
        let log = fs::read_to_string(log_path).unwrap();
        assert!(log.contains("stdout line"));
        assert!(log.contains("stderr line"));

        let prompt = fs::read_to_string(captured_prompt).unwrap();
        assert!(prompt.contains("/evolution-propose"));
        assert!(prompt.contains("Session metadata file:"));
        assert!(prompt.contains("stockfishComparison"));

        match previous_claude_bin {
            Some(value) => env::set_var("CLAUDE_BIN", value),
            None => env::remove_var("CLAUDE_BIN"),
        }
    }

    #[test]
    fn run_phase_returns_failed_exit_code() {
        let _guard = env_lock().lock().unwrap();
        let temp = tempdir().unwrap();
        let script_path = temp.path().join("fake-claude.sh");
        fs::write(&script_path, "#!/bin/sh\ncat >/dev/null\nexit 17\n").unwrap();
        make_executable(&script_path);

        let previous_claude_bin = env::var_os("CLAUDE_BIN");
        env::set_var("CLAUDE_BIN", &script_path);

        let config = sample_config(temp.path());
        let outcome = run_phase(&config).unwrap();
        assert_eq!(outcome, PhaseOutcome::Failed(17));

        match previous_claude_bin {
            Some(value) => env::set_var("CLAUDE_BIN", value),
            None => env::remove_var("CLAUDE_BIN"),
        }
    }

    #[test]
    fn run_phase_returns_timeout() {
        let _guard = env_lock().lock().unwrap();
        let temp = tempdir().unwrap();
        let script_path = temp.path().join("fake-claude.sh");
        fs::write(&script_path, "#!/bin/sh\ncat >/dev/null\nsleep 5\n").unwrap();
        make_executable(&script_path);

        let previous_claude_bin = env::var_os("CLAUDE_BIN");
        env::set_var("CLAUDE_BIN", &script_path);

        let mut config = sample_config(temp.path());
        config.phase_timeout_secs = 1;
        let outcome = run_phase(&config).unwrap();
        assert_eq!(outcome, PhaseOutcome::Timeout);

        match previous_claude_bin {
            Some(value) => env::set_var("CLAUDE_BIN", value),
            None => env::remove_var("CLAUDE_BIN"),
        }
    }
}
