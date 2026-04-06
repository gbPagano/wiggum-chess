# PRD: Chess Engine + Match System + Evolution Tracking

## Introduction

Build a complete chess engine development and testing pipeline. This includes: downloading Stockfish as a benchmark opponent, ensuring chess-runner works end-to-end for multi-game matches, creating a new `chess-engine` crate with a basic material+minimax evaluation, and implementing an evolution tracking system that logs match results over time and generates text-based reports showing engine progress.

## Goals

- Have a working Stockfish binary available for benchmarking
- Validate that chess-runner + chesslib support full engine-vs-engine match series with correct result tracking
- Create a new `chess-engine` workspace crate that implements the `Engine` trait both in-process and as a UCI binary
- Implement a basic engine using material evaluation + minimax search (fixed shallow depth ~3-4)
- Generate structured match result logs (CSV) and CLI-driven text summaries showing engine evolution over time

## User Stories

### US-001: Download and configure Stockfish
**Description:** As a developer, I want Stockfish available locally so that I can use it as a benchmark engine for testing matches.

**Acceptance Criteria:**
- [ ] Script or instructions to download Stockfish binary for the current platform
- [ ] Stockfish binary is placed in a known location (e.g., `engines/stockfish`)
- [ ] Stockfish responds correctly to UCI handshake when launched by chess-runner
- [ ] `engines/` directory is gitignored

### US-002: Validate chess-runner match series
**Description:** As a developer, I want to confirm that chess-runner correctly runs a series of N games between two engines, alternating colors, and produces an accurate win/draw/loss summary.

**Acceptance Criteria:**
- [ ] Running `chess-runner --engine1 X --engine2 Y --games N` completes without errors
- [ ] Colors alternate correctly between games (even=engine1 white, odd=engine1 black)
- [ ] Final summary correctly tallies wins for each engine and draws
- [ ] Match works with Stockfish as one of the engines (e.g., Stockfish vs Stockfish)

### US-003: Create chess-engine crate skeleton
**Description:** As a developer, I want a new `chess-engine` crate in the workspace so that I have a dedicated place to implement and iterate on our custom engine.

**Acceptance Criteria:**
- [ ] New `chess-engine/` directory added to workspace members in root `Cargo.toml`
- [ ] Crate depends on `chesslib` (path dependency)
- [ ] Crate compiles with `cargo build -p chess-engine`
- [ ] Basic project structure: `src/lib.rs` for engine logic, `src/main.rs` for UCI binary

### US-004: Implement material evaluation function
**Description:** As a developer, I want a function that scores a board position based on piece values so that the engine can compare positions.

**Acceptance Criteria:**
- [ ] Function `evaluate(board: &Board) -> i32` returns centipawn score from the side-to-move's perspective
- [ ] Standard piece values: Pawn=100, Knight=320, Bishop=330, Rook=500, Queen=900
- [ ] Score is positive when side-to-move has material advantage, negative when behind
- [ ] Checkmate returns a very large positive/negative value; stalemate returns 0
- [ ] Unit tests cover: starting position (equal ~0), positions with material imbalance, checkmate, stalemate

### US-005: Implement minimax search with fixed depth
**Description:** As a developer, I want a minimax search that explores moves to a fixed depth so that the engine can look ahead and choose strategically.

**Acceptance Criteria:**
- [ ] `search(board: &Board, depth: u8) -> (ChessMove, i32)` returns best move and score
- [ ] Uses negamax formulation (simplified minimax)
- [ ] Default search depth of 3-4 plies, configurable via `--depth` command-line flag
- [ ] At depth 0, returns the material evaluation
- [ ] Returns a legal move for any non-terminal position
- [ ] Unit tests: finds mate-in-1, avoids hanging a queen when possible

### US-006: Implement Engine trait (in-process)
**Description:** As a developer, I want the chess-engine to implement chesslib's `Engine` trait so that it can be used directly in-process with the match runner.

**Acceptance Criteria:**
- [ ] Struct (e.g., `MaterialEngine`) implements `Engine` trait from chesslib
- [ ] `go()` uses the minimax search to select a move
- [ ] `name()` returns a versioned name (e.g., "MaterialEngine v0.1") — version is included in CSV logs for tracking evolution
- [ ] Engine can complete a full game via `Match::run()` without panicking
- [ ] Integration test: MaterialEngine vs MaterialEngine completes a game with a valid result

### US-007: UCI protocol wrapper binary
**Description:** As a developer, I want `chess-engine` to produce a UCI-compatible binary so that it can be used with chess-runner and external UCI GUIs.

**Acceptance Criteria:**
- [ ] `cargo run -p chess-engine` starts a UCI loop reading from stdin
- [ ] Responds to `uci` with `id name`, `id author`, and `uciok`
- [ ] Responds to `isready` with `readyok`
- [ ] Handles `position startpos moves ...` and `position fen ... moves ...`
- [ ] Handles `go wtime btime winc binc` and responds with `bestmove <uci_move>`
- [ ] Handles `ucinewgame` and `quit`
- [ ] Works as an engine in chess-runner: `chess-runner --engine1 ./target/release/chess-engine --engine2 <stockfish>`

### US-008: CSV match result logging
**Description:** As a developer, I want chess-runner to append match results to a CSV file so that I can track engine performance over time.

**Acceptance Criteria:**
- [ ] New `--output <path>` flag on chess-runner to specify CSV output file
- [ ] Each match run appends one row with: timestamp, engine1_name, engine2_name, games_played, engine1_wins, engine2_wins, draws, engine1_win_rate
- [ ] Creates the file with a header row if it doesn't exist
- [ ] Appends without overwriting if file already exists
- [ ] CSV is well-formed and parseable by standard tools

### US-009: Evolution report CLI command
**Description:** As a developer, I want a CLI command that reads the CSV log and prints a text-based summary of engine evolution so that I can quickly see progress.

**Acceptance Criteria:**
- [ ] New subcommand or flag on chess-runner: `chess-runner report --input <csv_path>`
- [ ] Displays a formatted table with: date, opponent, games, wins, losses, draws, win rate
- [ ] Shows overall summary: total games, total win rate, best/worst matchup
- [ ] If the CSV has multiple entries for the same engine pair, shows trend (improving/declining)
- [ ] Handles empty or missing CSV file gracefully with a clear message

## Functional Requirements

- FR-1: A `download-stockfish.sh` script shall download the appropriate Stockfish binary for the host OS/arch and place it in `engines/stockfish`
- FR-2: `chess-runner` shall run N games between two UCI engines, alternating colors, and print a summary of results
- FR-3: The `chess-engine` crate shall be a workspace member with both library and binary targets
- FR-4: The evaluation function shall score positions using standard piece values (P=100, N=320, B=330, R=500, Q=900)
- FR-5: The search function shall use negamax with a configurable fixed depth (default 3-4)
- FR-6: `chess-engine` shall implement the `Engine` trait for in-process usage
- FR-7: `chess-engine` binary shall implement the UCI protocol (uci, isready, position, go, quit)
- FR-8: `chess-runner --output <csv>` shall log match results in CSV format with timestamps
- FR-9: `chess-runner report --input <csv>` shall display a text-based evolution report

## Non-Goals

- No alpha-beta pruning or advanced search optimizations in the initial implementation
- No opening book or endgame tablebase support
- No Elo calculation (just win/draw/loss rates)
- No GUI or web-based visualization — text reports only
- No time management strategy — engine uses all available time equally
- No transposition table or move ordering
- No support for chess variants (standard chess only)

## Technical Considerations

- The `Engine` trait in chesslib is async (`async_trait`) — the in-process implementation will need to be async-compatible even though search is synchronous
- UCI protocol parsing in `chess-engine` can reuse patterns from `chesslib/src/uci_engine.rs` (which handles the client side)
- Minimax at depth 4 with ~30 legal moves per position = ~810,000 nodes — should be fast enough without optimization given chesslib's performant move generation
- CSV logging should use a well-known Rust crate (e.g., `csv`) for robustness
- The `chess-runner` binary currently uses `clap` — extend with subcommands for the report feature

## Success Metrics

- Stockfish can be downloaded and used in matches with a single script
- chess-engine completes a 10-game match against Stockfish without crashes or protocol errors
- Match results are correctly logged to CSV after each run
- Evolution report correctly shows win rates across multiple logged match runs
- Engine finds mate-in-1 in all standard mate-in-1 puzzles
- Full test suite passes: `cargo test --workspace`

## Resolved Questions

- **Minimax depth configuration:** Configurable via command-line flags (not UCI options).
- **Engine versioning in CSV:** Yes — engine name in CSV includes version (e.g., "MaterialEngine v0.1") to track different engine versions over time.
- **Report filtering:** No — the report command shows all data, no filtering by date range or opponent.
