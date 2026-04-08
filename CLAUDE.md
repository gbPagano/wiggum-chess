# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Cargo workspace containing:
- **`chesslib`** — Rust chess move generator library focused on performance, using bitboard representation with magic bitboard sliding piece attack generation.
- **`chess-runner`** — Binary crate for running engine matches and CLI tooling.

## Build & Test Commands

```bash
cargo build --workspace                      # debug build (still opt-level=3)
cargo build --release --workspace            # release build (LTO, single codegen unit)
cargo test --workspace                       # run all tests
cargo test -p chesslib movegen_perft_5       # run a single test by name
cargo run -p chess-runner -- 6              # run perft to depth 6
cargo bench --bench perft -p chesslib        # criterion perft benchmarks (depths 1-7)
cargo bench --bench alloc -p chesslib        # allocation benchmarks (divan)
```

Note: `gen_files::magics::name` is an intentional developer scratchpad test with `assert!(false)` — always fails. For full-workspace runs, skip it with `cargo test --workspace -- --skip gen_files::magics::name`.

## Architecture

### Workspace Structure

- `chesslib/` — Library crate (move generation, board state, etc.)
- `chess-runner/` — Binary crate depending on chesslib
- `Cargo.toml` — Workspace root (profiles defined here)

### Benchmarking Reports

- `scripts/benchmark-version.sh` post-processes the last appended `results.csv` row to add the Stockfish level suffix (1500/2000/2500/max), because `chess-runner match` records only the opponent engine's UCI-reported name in CSV output.
- If release binaries fail to start on the local machine because of GLIBC mismatch, use the freshly built debug binaries in `target/debug/` for local benchmarking and report generation.
- Promoted engine versions are tracked in two synchronized places: `[package].version` in `chess-engine/Cargo.toml`, `chess-runner/Cargo.toml`, and `chesslib/Cargo.toml`, plus `chess-engine/versions/<tag>/` for release artifacts (`CHANGES.md`, `report.md`). Keep `v<major>.<minor>` tags aligned with Cargo semver `<major>.<minor>.0`.
- `scripts/evolution-loop.sh` is responsible for normalizing optional propose-time inputs before invoking Claude skills. When using an operator-supplied ideas checklist, keep the canonical path in both `session.env` and `iteration.json`, but clear it entirely when the file has no unchecked `- [ ]` entries so the propose phase falls back to self-generated ideas without extra branching.
- Proposal provenance for checklist-backed iterations belongs in `iteration.json.ideas`: the propose phase must write `proposalSource` plus the exact `selectedIdea`, and the orchestration loop should flip the checklist entry to `- [x]` only after the iteration reaches a tested terminal outcome.
- Candidate versioning for evolution runs is orchestration-owned: after the propose phase sets `ideas.proposalSource`, `scripts/evolution-loop.sh` computes the candidate tag (`minor` bump for `self_proposed`, `major` bump for `user_ideas_file`), synchronizes all three Cargo manifest versions, and records the generated `wiggum-engine` debug binary path in both `session.env` and `iteration.json` before benchmarking.
- When syncing candidate Cargo versions by string replacement, update only the package-level `version = "..."` line in each manifest so dependency versions remain untouched.
- Baseline selection for evolution runs is artifact-owned: `--baseline-version` must resolve to `chess-engine/versions/<tag>/`, and the loop records `accepted_baseline_path` / `accepted_baseline_binary` in `session.env` plus `baselinePath` / `baselineBinary` in `iteration.json`. The git ref remains internal orchestration state only for creating candidate worktrees.
- Accepted promotions must copy the built candidate binary into `chess-engine/versions/<tag>/wiggum-engine` before the candidate worktree is removed, then update both `decision.promotion` in `iteration.json` and the session metadata from that stored artifact path so subsequent iterations do not depend on the discarded worktree.
- Treat `session.env` `active_baseline_version` / `active_baseline_path` / `active_baseline_binary` as the operator-facing contract for the currently promoted baseline; keep them synchronized with the accepted baseline fields whenever a promotion happens, and preserve them unchanged for rejected, failed, or inconclusive iterations.
- Inconclusive benchmark follow-up is artifact-driven too: seed each iteration with `iteration.json.stockfishComparison` plus `artifacts.stockfishComparison`, pointing at the active baseline report and per-iteration comparison files, so benchmark/decision phases can record an extra candidate-vs-Stockfish check, whether that follow-up changed the recommendation, or an explicit limitation without inventing paths ad hoc.
- Positive-signal promotion is a narrow decision-time exception: only after both the direct benchmark and Stockfish follow-up remain inconclusive may the decision phase set `iteration.json.stockfishComparison.positiveSignal`; promotion is allowed from otherwise inconclusive evidence only when that structure says the signal is present and the same evidence is repeated in `decision.md` and `iteration.json.decision.evidence`.
- `scripts/evolution-loop.sh` defaults to quiet Ralph-style phase execution, but `--verbose` should stream live Claude skill output with `tee` while preserving the same per-phase log artifacts and iteration outcomes as quiet mode.
- The evolution-loop correctness gate should run only `cargo build --workspace` and `cargo test --workspace -- --skip gen_files::magics::name` inside the candidate workspace, stop on the first failing check, write `correctness/results.md`, and mirror the result into `iteration.json.correctness` with `benchmarkEligible` set only when both checks pass.

### Build-time Code Generation (`chesslib/src/build.rs`, `chesslib/src/gen_files/`)

The build script generates lookup tables into `$OUT_DIR/magic_file.rs`, which is `include!()`'d by `chesslib/src/magic.rs`. Generated tables include: magic numbers for sliding pieces, rays, between/line bitboards, knight/king/pawn move tables, and chessboard utility constants. The `src/gen_files/` modules contain the generation logic and are compiled separately for the build script (they re-declare shared types like `bitboard`, `square`, etc.).

### Core Types

- **`BitBoard`** (`chesslib/src/bitboard.rs`) — `u64` wrapper representing a set of squares. All piece positions and move sets are bitboards.
- **`Board`** (`chesslib/src/board.rs`) — Full board state: piece/color/combined bitboards, side to move, castling rights, en passant, and precomputed pinned/checkers bitboards. Immutable move application via `make_move()` (returns new Board). Parses/displays FEN strings.
- **`Square`**, **`Rank`**, **`File`**, **`Color`**, **`Piece`** — Small enum/newtype wrappers with index conversions.

### Move Generation (`chesslib/src/piece_moves.rs`, `chesslib/src/movegen.rs`)

Legal move generation uses a trait-based design:
- `PieceMoves` trait with `pseudo_legals()` and `legals()` methods, implemented per piece type (`RookMoves`, `BishopMoves`, etc.).
- `CheckStatus` trait (`InCheck`/`NotInCheck`) used as a const generic parameter to specialize check-aware move generation at compile time.
- Pinned pieces are restricted to movement along their pin line. In double check, only king moves are generated.
- `MoveGen` is an iterator over `ChessMove` that lazily expands `BitBoardMove` entries (bitboard of destinations per source square).

### Magic Bitboards (`chesslib/src/magic.rs`)

Sliding piece attacks (rook/bishop) use magic bitboard lookup. `get_rook_moves()`/`get_bishop_moves()` use unsafe unchecked indexing for performance. All other piece move lookups are also unsafe table reads.

## Testing

Correctness is validated through perft tests (`chesslib/src/movegen.rs` tests) — comparing node counts at various depths against known-correct values for many positions. These are the primary correctness tests.
