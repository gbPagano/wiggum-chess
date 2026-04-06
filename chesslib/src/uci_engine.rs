use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

use crate::board::Board;
use crate::chess_move::ChessMove;
use crate::engine::{Engine, TimeControl};
use crate::game::Game;

/// A UCI-compatible chess engine running as a subprocess.
///
/// Communicates with the engine via stdin/stdout using the UCI protocol.
/// On construction, performs the UCI handshake (`uci` → `uciok`) and
/// synchronization (`isready` → `readyok`).
pub struct UciEngine {
    name: String,
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    response_timeout: Duration,
    current_board: Option<Board>,
}

impl UciEngine {
    /// Spawn a UCI engine at `path` and complete the UCI handshake.
    ///
    /// `timeout_ms` is the maximum milliseconds to wait for any single
    /// engine response (e.g. `uciok`, `readyok`, `bestmove`).
    pub async fn new(path: impl AsRef<Path>, timeout_ms: u64) -> Result<Self> {
        let response_timeout = Duration::from_millis(timeout_ms);

        let mut child = Command::new(path.as_ref())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .with_context(|| format!("failed to spawn engine at {:?}", path.as_ref()))?;

        let stdin = child
            .stdin
            .take()
            .context("failed to acquire engine stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("failed to acquire engine stdout")?;
        let stdout = BufReader::new(stdout).lines();

        let mut engine = Self {
            name: String::new(),
            child,
            stdin,
            stdout,
            response_timeout,
            current_board: None,
        };

        // Send "uci" and collect engine name from "id name ..." line.
        engine.send_line("uci").await?;
        engine.name = engine.read_until_uciok().await?;

        // Synchronize with isready/readyok.
        engine.send_line("isready").await?;
        engine.read_until_readyok().await?;

        Ok(engine)
    }

    async fn send_line(&mut self, cmd: &str) -> Result<()> {
        self.stdin.write_all(cmd.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    /// Read the next line from the engine's stdout, with a timeout.
    async fn next_line_with_timeout(&mut self) -> Result<String> {
        timeout(self.response_timeout, self.stdout.next_line())
            .await
            .context("engine response timed out")?
            .context("engine stdout I/O error")?
            .context("engine stdout closed unexpectedly")
    }

    /// Read lines until "uciok"; collect engine name from "id name ..." lines.
    async fn read_until_uciok(&mut self) -> Result<String> {
        let mut name = String::new();
        loop {
            let line = self.next_line_with_timeout().await?;
            if let Some(n) = line.strip_prefix("id name ") {
                name = n.to_string();
            } else if line.trim() == "uciok" {
                return Ok(name);
            }
        }
    }

    /// Read lines until "readyok", discarding intermediate lines.
    async fn read_until_readyok(&mut self) -> Result<()> {
        loop {
            let line = self.next_line_with_timeout().await?;
            if line.trim() == "readyok" {
                return Ok(());
            }
        }
    }

    /// Read lines until "bestmove <move> ...", returning the move token.
    async fn read_until_bestmove(&mut self) -> Result<String> {
        loop {
            let line = self.next_line_with_timeout().await?;
            if let Some(rest) = line.strip_prefix("bestmove ") {
                // Ignore optional "ponder <move>" suffix.
                let mv_str = rest.split_whitespace().next().unwrap_or("").to_string();
                return Ok(mv_str);
            }
        }
    }

    /// Send a UCI `setoption` command to configure the engine.
    ///
    /// Sends `setoption name <name> value <value>` and flushes stdin.
    /// Per the UCI spec, `setoption` has no reply from the engine.
    pub async fn set_option(&mut self, name: &str, value: &str) -> Result<()> {
        let cmd = format!("setoption name {} value {}", name, value);
        self.send_line(&cmd).await
    }
}

#[async_trait]
impl Engine for UciEngine {
    async fn name(&self) -> String {
        self.name.clone()
    }

    async fn new_game(&mut self) {
        let _ = self.send_line("ucinewgame").await;
        let _ = self.send_line("isready").await;
        let _ = self.read_until_readyok().await;
        self.current_board = None;
    }

    async fn set_position(&mut self, game: &Game) {
        let moves: Vec<String> = game.moves().iter().map(|m| m.to_uci()).collect();
        let cmd = if moves.is_empty() {
            "position startpos".to_string()
        } else {
            format!("position startpos moves {}", moves.join(" "))
        };
        let _ = self.send_line(&cmd).await;
        self.current_board = Some(game.board().clone());
    }

    async fn go(&mut self, time_control: &TimeControl) -> ChessMove {
        let mut cmd = format!(
            "go wtime {} btime {} winc {} binc {}",
            time_control.wtime, time_control.btime, time_control.winc, time_control.binc
        );
        if let Some(movestogo) = time_control.movestogo {
            cmd.push_str(&format!(" movestogo {}", movestogo));
        }
        let _ = self.send_line(&cmd).await;

        let mv_str = self
            .read_until_bestmove()
            .await
            .expect("failed to read bestmove from engine");

        let board = self.current_board.clone().unwrap_or_default();
        ChessMove::from_uci(&mv_str, &board).expect("engine returned invalid UCI move")
    }

    async fn quit(&mut self) {
        let _ = self.send_line("quit").await;
        // Wait up to 5 s for the process to exit gracefully.
        let _ = timeout(Duration::from_secs(5), self.child.wait()).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::TimeControl;
    use crate::game::Game;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::OnceLock;

    static MOCK_ENGINE: OnceLock<PathBuf> = OnceLock::new();

    /// Return the path to the mock UCI engine shell script, creating it once.
    fn mock_engine_path() -> &'static PathBuf {
        MOCK_ENGINE.get_or_init(|| {
            let path = PathBuf::from("/tmp/mock_uci_engine_chesslib.sh");
            let script = r#"#!/bin/sh
while IFS= read -r line; do
    case "$line" in
        uci)
            printf 'id name MockEngine\n'
            printf 'uciok\n'
            ;;
        isready)
            printf 'readyok\n'
            ;;
        ucinewgame)
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

    #[tokio::test]
    async fn test_uci_engine_handshake_and_name() {
        let path = mock_engine_path();
        let mut engine = UciEngine::new(path, 2000).await.unwrap();
        assert_eq!(engine.name().await, "MockEngine");
        engine.quit().await;
    }

    #[tokio::test]
    async fn test_uci_engine_new_game() {
        let path = mock_engine_path();
        let mut engine = UciEngine::new(path, 2000).await.unwrap();
        engine.new_game().await;
        engine.quit().await;
    }

    #[tokio::test]
    async fn test_uci_engine_set_position_and_go() {
        let path = mock_engine_path();
        let mut engine = UciEngine::new(path, 2000).await.unwrap();
        let game = Game::new();
        engine.set_position(&game).await;
        let tc = TimeControl::new(5000, 5000, 100, 100, None);
        let mv = engine.go(&tc).await;
        // Mock engine always returns e2e4, which is a legal opening move.
        assert_eq!(mv.to_uci(), "e2e4");
        engine.quit().await;
    }

    #[tokio::test]
    async fn test_uci_engine_go_with_movestogo() {
        let path = mock_engine_path();
        let mut engine = UciEngine::new(path, 2000).await.unwrap();
        let game = Game::new();
        engine.set_position(&game).await;
        let tc = TimeControl::new(40000, 40000, 0, 0, Some(40));
        let mv = engine.go(&tc).await;
        assert_eq!(mv.to_uci(), "e2e4");
        engine.quit().await;
    }

    #[tokio::test]
    async fn test_uci_engine_implements_engine_trait() {
        let path = mock_engine_path();
        let mut engine: Box<dyn Engine> = Box::new(UciEngine::new(path, 2000).await.unwrap());
        assert_eq!(engine.name().await, "MockEngine");
        engine.new_game().await;
        engine.quit().await;
    }

    #[tokio::test]
    async fn test_uci_engine_spawn_nonexistent_fails() {
        let result = UciEngine::new("/nonexistent/engine/binary", 1000).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_option_sends_without_error() {
        // The mock engine silently ignores unknown commands (unmatched case branches).
        let path = mock_engine_path();
        let mut engine = UciEngine::new(path, 2000).await.unwrap();
        // setoption has no reply per UCI spec — just verify it doesn't error.
        engine.set_option("Skill Level", "10").await.unwrap();
        engine.quit().await;
    }
}
