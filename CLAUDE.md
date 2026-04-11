# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Build
- `cargo build --workspace`
- `cargo build --release -p chess-engine`
- `cargo build --release -p chess-runner`
- `cargo build --release -p perft-bench`

### Test
- Full workspace correctness run used by the evolution loop: `cargo test --workspace -- --skip gen_files::magics::name`
- Single crate: `cargo test -p chesslib`
- Single test: `cargo test -p chesslib <test_name> -- --nocapture`
- Engine crate only: `cargo test -p chess-engine`
- Evolution loop crate only: `cargo test -p evolution-loop`

### Benchmarks and perf tools
- Quick perft bench alias from `.cargo/config.toml`: `cargo quick-bench`
- Direct perft bench: `cargo bench -p chesslib --bench perft -- --quick --quiet`
- Cross-library perft benchmark script: `./perft-bench/bench.sh --position starting`
  - Optional flags: `--simple`, `--python`, `--position <starting|kiwipete|promotions|captures>`
  - Requires `hyperfine`; Stockfish must be on `PATH` or provided via `STOCKFISH_BIN`
- Version benchmark orchestration: `./scripts/benchmark-version.sh --version <tag> --engine <path> --prev-engine <path>`

### Run binaries
- Run the UCI engine: `cargo run -p chess-engine -- --depth 5`
- Run an engine match: `cargo run -p chess-runner -- match --engine1 <path> --engine2 <path> --games 2`
- Run an SPRT match: `cargo run -p chess-runner -- sprt --engine1 <path> --engine2 <path>`
- Start the evolution orchestrator: `cargo run -p evolution-loop -- start --baseline-version <vX.Y>`
- Resume an evolution session: `cargo run -p evolution-loop -- resume --session <tasks/evolution-runs/...>`

## Architecture

This is a Rust workspace centered on `chesslib`, with thin binaries layered on top of it.

### `chesslib`: core chess model, move generation, game rules, and engine integrations
- `chesslib/src/board.rs` is the core position type. It stores piece/color bitboards, side to move, castling rights, en passant square, halfmove clock, cached pin/checker bitboards, and a Zobrist hash.
- `chesslib/src/build.rs` generates magic-bitboard tables, rays, between/line tables, pawn/king/knight move tables, and Zobrist keys at build time. `chesslib/src/magic.rs` includes the generated file from `OUT_DIR`.
- `chesslib/src/movegen.rs` is the legal move entry point. It uses the board’s cached `checkers_bitboard` / `pinned_bitboard` state plus piece-specific generators from `piece_moves.rs` rather than filtering a pseudo-legal list afterward.
- `chesslib/src/game.rs` wraps `Board` with move history, repetition tracking, optional clocks, and result detection. `Game` is what higher-level match code passes to engines.
- `chesslib/src/engine.rs` defines the async `Engine` trait. `chesslib/src/uci_engine.rs` implements it for subprocess UCI engines; `chesslib/src/analysis.rs` is a separate synchronous Stockfish wrapper used for evaluation/extraction workflows.
- `chesslib/src/match_runner.rs` orchestrates games between two `Engine` implementations and applies match-level rules such as auto-drawing on threefold repetition.
- `chesslib/src/pgn.rs` parses PGN into `ChessMove`s by replaying SAN moves on top of `Board`/`MoveGen`.

### `chess-engine`: the in-repo engine binary
- `chess-engine/src/main.rs` is a minimal UCI loop. It parses `position`, answers `uci` / `isready`, and on `go` calls the in-process search.
- `chess-engine/src/search.rs` is currently a plain negamax alpha-beta search over `MoveGen::new_legal(board)`.
- `chess-engine/src/eval.rs` is a simple material + piece-square-table evaluator from the side-to-move perspective.
- Time-control fields from UCI are parsed for compatibility, but the engine currently searches by fixed depth (`--depth`) rather than doing time management.

### `chess-runner`: benchmarking and match orchestration CLI
- `chess-runner` is the operational CLI for engine-vs-engine matches, SPRT runs, version reports, PGN replay, and balanced-position extraction.
- It reuses `chesslib::match_runner::Match` and `chesslib::uci_engine::UciEngine`; most evaluation infrastructure sits here rather than in `chess-engine`.
- `chess-runner/src/opening_book.rs` defines the opening-book format: one UCI move sequence per non-comment line, validated by replaying each line from the starting position.
- `scripts/benchmark-version.sh` builds on `chess-runner`; because `chess-runner match` cannot send startup `setoption` commands, the script creates temporary Stockfish wrapper scripts for skill-level benchmarking.

### `evolution-loop`: autonomous engine iteration orchestrator
- `evolution-loop` drives propose → implement → validate → benchmark → decide sessions and persists state in `session.env` plus per-iteration `iteration.json`.
- It creates isolated candidate git worktrees, runs phase commands, and promotes accepted candidates.
- `evolution-loop/src/versioning.rs` updates the package version in exactly `chess-engine`, `chess-runner`, and `chesslib` when a candidate version changes.
- `evolution-loop/src/worktree.rs` owns git worktree creation/removal and candidate commit handling.
- The worker-specific contract for this flow lives in `.claude/evolution/CLAUDE.md`; follow it when working inside evolution sessions.

### Supporting crates and directories
- `perft-bench` is a separate benchmark binary that compares `chesslib` against other move-generation implementations (`chesslib-simple`, `chess`, `shakmaty`, optionally python-chess and Stockfish perft).
- `chesslib-simple` is a simpler chess implementation used mainly as a comparison/reference target.
- `chess-engine/versions/<tag>/` stores released version artifacts (`CHANGES.md`, benchmark CSVs, generated reports).
- `scripts/ralph/CLAUDE.md` contains the workflow contract for Ralph-style PRD-driven autonomous work.

## Repo-specific notes
- There is no top-level README right now; the nested CLAUDE files under `.claude/evolution/` and `scripts/ralph/` are the main workflow-specific instructions.
- The workspace uses aggressive optimization even in dev/test profiles (`opt-level = 3`), so compile behavior is closer to performance-oriented debugging than default Cargo dev builds.
- If you touch magic move generation or generated attack tables, remember that the data comes from `chesslib/src/build.rs`, not from checked-in source files alone.
- If you need full-workspace validation for engine-evolution work, prefer the same command the orchestrator uses: `cargo build --workspace` followed by `cargo test --workspace -- --skip gen_files::magics::name`.
