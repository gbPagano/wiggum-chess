use std::error::Error;
use std::fmt;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

const LINE_TIMEOUT: Duration = Duration::from_secs(5);

/// Error type for Stockfish analysis operations.
#[derive(Debug)]
pub enum AnalysisError {
    SpawnFailed(String),
    HandshakeFailed(String),
    Timeout,
    ProcessDied,
    IoError(String),
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalysisError::SpawnFailed(msg) => write!(f, "Failed to spawn Stockfish: {}", msg),
            AnalysisError::HandshakeFailed(msg) => write!(f, "UCI handshake failed: {}", msg),
            AnalysisError::Timeout => write!(f, "Stockfish analysis timed out"),
            AnalysisError::ProcessDied => write!(f, "Stockfish process died unexpectedly"),
            AnalysisError::IoError(msg) => write!(f, "I/O error communicating with Stockfish: {}", msg),
        }
    }
}

impl Error for AnalysisError {}

/// A thin wrapper around a running Stockfish process with piped stdin/stdout.
///
/// On construction, spawns Stockfish and performs the UCI handshake
/// (`uci` → wait for `uciok`, then `isready` → wait for `readyok`).
pub struct StockfishProcess {
    child: Child,
    stdin: BufWriter<std::process::ChildStdin>,
    line_rx: Receiver<Result<String, AnalysisError>>,
}

impl StockfishProcess {
    /// Spawn Stockfish at `path` and perform the UCI handshake.
    pub fn new(path: &Path) -> Result<StockfishProcess, AnalysisError> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| AnalysisError::SpawnFailed(e.to_string()))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AnalysisError::SpawnFailed("failed to acquire stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AnalysisError::SpawnFailed("failed to acquire stdout".to_string()))?;

        // Spawn a reader thread that forwards lines via channel.
        let (tx, rx) = mpsc::channel::<Result<String, AnalysisError>>();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        if tx.send(Ok(l)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(AnalysisError::IoError(e.to_string())));
                        break;
                    }
                }
            }
        });

        let mut sf = StockfishProcess {
            child,
            stdin: BufWriter::new(stdin),
            line_rx: rx,
        };

        // UCI handshake.
        sf.send_line("uci")
            .map_err(|e| AnalysisError::HandshakeFailed(e.to_string()))?;
        sf.read_until_token("uciok")
            .map_err(|_| AnalysisError::HandshakeFailed("never received uciok".to_string()))?;

        sf.send_line("isready")
            .map_err(|e| AnalysisError::HandshakeFailed(e.to_string()))?;
        sf.read_until_token("readyok")
            .map_err(|_| AnalysisError::HandshakeFailed("never received readyok".to_string()))?;

        Ok(sf)
    }

    fn send_line(&mut self, cmd: &str) -> Result<(), AnalysisError> {
        write!(self.stdin, "{}\n", cmd)
            .map_err(|e| AnalysisError::IoError(e.to_string()))?;
        self.stdin
            .flush()
            .map_err(|e| AnalysisError::IoError(e.to_string()))?;
        Ok(())
    }

    fn next_line(&self) -> Result<String, AnalysisError> {
        self.line_rx
            .recv_timeout(LINE_TIMEOUT)
            .map_err(|e| match e {
                mpsc::RecvTimeoutError::Timeout => AnalysisError::Timeout,
                mpsc::RecvTimeoutError::Disconnected => AnalysisError::ProcessDied,
            })?
    }

    fn read_until_token(&self, token: &str) -> Result<(), AnalysisError> {
        loop {
            let line = self.next_line()?;
            if line.trim() == token {
                return Ok(());
            }
        }
    }
}

impl Drop for StockfishProcess {
    fn drop(&mut self) {
        // Best-effort quit; ignore errors on cleanup.
        let _ = write!(self.stdin, "quit\n");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

/// Send a FEN to a running Stockfish process and return the centipawn score.
///
/// Returns `Ok(Some(cp))` where `cp` is white-positive centipawns from the
/// last `info score cp` line received before `bestmove`.
///
/// Returns `Ok(None)` when Stockfish reports only mate scores (no `cp` line).
///
/// Returns `Err(AnalysisError)` if Stockfish dies, times out, or an I/O error
/// occurs.
pub fn analyze_fen(
    sf: &mut StockfishProcess,
    fen: &str,
    depth: u8,
) -> Result<Option<i32>, AnalysisError> {
    sf.send_line(&format!("position fen {}", fen))?;
    sf.send_line(&format!("go depth {}", depth))?;

    let mut last_cp: Option<i32> = None;

    loop {
        let line = sf.next_line()?;

        if line.starts_with("bestmove") {
            return Ok(last_cp);
        }

        // Parse "info ... score cp <X> ..." lines.
        // Ignore "score mate <X>" lines — they do not update last_cp.
        if line.starts_with("info") {
            if let Some(pos) = line.find("score cp ") {
                let rest = &line[pos + "score cp ".len()..];
                if let Some(token) = rest.split_whitespace().next() {
                    if let Ok(cp) = token.parse::<i32>() {
                        last_cp = Some(cp);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::OnceLock;

    static MOCK_SF: OnceLock<PathBuf> = OnceLock::new();

    /// A mock "Stockfish" binary that handles the UCI handshake and responds to
    /// `go depth N` with `bestmove e2e4` (no cp line).
    fn mock_sf_path() -> &'static PathBuf {
        MOCK_SF.get_or_init(|| {
            let path = PathBuf::from("/tmp/mock_stockfish_analysis.sh");
            let script = r#"#!/bin/sh
while IFS= read -r line; do
    case "$line" in
        uci)
            printf 'uciok\n'
            ;;
        isready)
            printf 'readyok\n'
            ;;
        position*)
            ;;
        go*)
            printf 'bestmove e2e4\n'
            ;;
        quit)
            exit 0
            ;;
    esac
done
"#;
            fs::write(&path, script).unwrap();
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).unwrap();
            path
        })
    }

    #[test]
    fn test_stockfish_process_handshake() {
        let path = mock_sf_path();
        let sf = StockfishProcess::new(path).expect("handshake should succeed");
        drop(sf);
    }

    #[test]
    fn test_analyze_fen_no_cp_returns_none() {
        let path = mock_sf_path();
        let mut sf = StockfishProcess::new(path).expect("handshake should succeed");
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let result = analyze_fen(&mut sf, fen, 1).expect("analyze_fen should not panic");
        assert_eq!(result, None, "mock returns no cp line so result should be None");
    }

    #[test]
    fn test_analyze_fen_with_cp_score() {
        // A mock that returns a cp score line before bestmove.
        let path = PathBuf::from("/tmp/mock_sf_with_cp.sh");
        let script = r#"#!/bin/sh
while IFS= read -r line; do
    case "$line" in
        uci)    printf 'uciok\n' ;;
        isready) printf 'readyok\n' ;;
        position*) ;;
        go*)
            printf 'info depth 1 score cp 42 nodes 100\n'
            printf 'bestmove e2e4\n'
            ;;
        quit) exit 0 ;;
    esac
done
"#;
        fs::write(&path, script).unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();

        let mut sf = StockfishProcess::new(&path).expect("handshake should succeed");
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let result = analyze_fen(&mut sf, fen, 1).expect("analyze_fen should succeed");
        assert_eq!(result, Some(42));
    }

    #[test]
    fn test_spawn_nonexistent_fails() {
        let result = StockfishProcess::new(Path::new("/nonexistent/stockfish"));
        assert!(result.is_err());
    }
}
