# PRD: Wiggum Engine — Verbose Mode, Rename, and Evolution Tracking

## Introduction

This PRD covers a set of improvements to the chess engine pipeline: adding a `--verbose` flag to
chess-runner to control output verbosity, renaming the engine from `MaterialEngine` to
`Wiggum Engine`, establishing a versioned folder structure inside `chess-engine/versions/` to track
evolution over time, implementing SPRT (Sequential Probability Ratio Test) for statistically
rigorous version comparisons, creating a benchmark shell script that tests each new version against
the previous version and Stockfish at four difficulty levels, and a `version-report` subcommand in
chess-runner that generates a rich markdown report per version.

The current engine version is **v0.1** — this PRD also covers registering it as the first official
versioned snapshot.

## Goals

- chess-runner output is controllable: verbose shows board+moves, silent shows only per-game
  results + running score and a final summary
- Engine name is consistently "Wiggum Engine" across all code, UCI responses, and CSV logs
- A structured `chess-engine/versions/` folder exists with a v0.1 snapshot report
- SPRT is available as a chess-runner match mode to quickly determine if a new version is stronger
- A script automates benchmarking a new version (vs previous + vs Stockfish at 1500/2000/2500/max)
- A `version-report` subcommand generates a self-contained markdown report from match CSV data

## Background: SPRT

SPRT (Sequential Probability Ratio Test) is the standard method used by engine development
communities (Stockfish, Leela) to determine if a new version is an improvement. Instead of running
a fixed number of games, SPRT runs games until it can statistically confirm or reject the hypothesis
that the new version is stronger by a given Elo margin.

Key parameters:
- **H0**: engines are equal (Elo difference = `elo0`, typically 0)
- **H1**: new engine is stronger (Elo difference = `elo1`, typically 5–10)
- **alpha** (false positive rate): 0.05
- **beta** (false negative rate): 0.05
- **LLR** (Log-Likelihood Ratio): computed after each game, test stops when LLR crosses upper bound
  (H1 accepted = improvement confirmed) or lower bound (H0 accepted = no improvement)

LLR bounds: lower = `ln(beta / (1 - alpha))`, upper = `ln((1 - beta) / alpha)`

## User Stories

### US-010: Add `--verbose` flag to chess-runner match command

**Description:** As a developer, I want to control how much chess-runner prints during a match so
that I can run quiet batch matches or verbose single-game debugging.

**Acceptance Criteria:**
- [ ] New `--verbose` flag added to `MatchArgs` in chess-runner
- [ ] Without `--verbose`: each game prints one line per game result + running score (e.g.,
  `Game 3/10: White wins | Score: 2-1-0`) and a final summary at the end; board and move lines are
  suppressed
- [ ] With `--verbose`: existing behavior preserved — board diagram and move line printed on every
  move, game result printed at end of each game
- [ ] `PrintObserver` is replaced by two observer types (or a configurable single type) that
  implement the respective behaviors
- [ ] The current always-verbose `PrintObserver` is removed or refactored; verbose is now opt-in
- [ ] `cargo test --workspace` passes

### US-011: Rename MaterialEngine to Wiggum Engine

**Description:** As a developer, I want the engine to be consistently named "Wiggum Engine" so that
match logs and UCI GUIs display the correct identity.

**Acceptance Criteria:**
- [ ] `MaterialEngine` struct in `chess-engine/src/engine.rs` renamed to `WiggumEngine`
- [ ] `name()` returns `"Wiggum Engine v0.1"`
- [ ] UCI binary (`chess-engine/src/main.rs`) responds to `uci` with `id name Wiggum Engine v0.1`
  and `id author chess-ic`
- [ ] `--about` text in the CLI updated from "MaterialEngine" to "Wiggum Engine"
- [ ] All tests referencing `MaterialEngine` or `material_engine` are updated
- [ ] `cargo test --workspace` passes

### US-012: Create `chess-engine/versions/` folder structure

**Description:** As a developer, I want a dedicated folder structure to store per-version snapshots
and reports so that I can track what changed and how each version performed over time.

**Acceptance Criteria:**
- [ ] Directory `chess-engine/versions/v0.1/` exists and is committed
- [ ] A `chess-engine/versions/v0.1/CHANGES.md` file exists documenting what this version is:
  initial implementation, material evaluation + negamax depth-4, no alpha-beta
- [ ] A `chess-engine/versions/README.md` exists explaining the folder convention (one subfolder
  per version, CHANGES.md + report.md per version)
- [ ] `.gitkeep` or placeholder used if no benchmark report exists yet for v0.1

### US-013: SPRT match mode in chess-runner

**Description:** As a developer, I want a SPRT-based match mode so that I can efficiently determine
whether a new engine version is an improvement without running an arbitrary fixed number of games.

**Acceptance Criteria:**
- [ ] New subcommand `chess-runner sprt` with arguments: `--engine1`, `--engine2`, `--time`,
  `--inc`, `--elo0` (default 0), `--elo1` (default 5), `--alpha` (default 0.05), `--beta`
  (default 0.05), `--timeout`, optional `--output` (CSV)
- [ ] After each completed game, LLR is recomputed and printed: e.g.,
  `[Game 14] LLR: 1.23 / [-2.94, 2.94] | W:6 D:3 L:5`
- [ ] Match stops automatically when LLR crosses either bound
- [ ] Final output states clearly: `SPRT Result: H1 accepted (improvement confirmed)` or
  `SPRT Result: H0 accepted (no improvement detected)`
- [ ] LLR formula uses the pentanomial or trinomial approximation:
  wins/draws/losses mapped to Elo via standard formula
  (`score = (wins + 0.5*draws) / total`, then `elo_diff = -400 * log10(1/score - 1)`)
- [ ] If `--output` is provided, appends one CSV row per completed SPRT run with final counts and
  result
- [ ] `cargo test --workspace` passes (unit test for LLR boundary computation with known values)

### US-017: Add `set_option` to `UciEngine` in chesslib

**Description:** As a developer, I want to send arbitrary UCI `setoption` commands to a subprocess
engine so that I can configure Stockfish skill levels (and any other UCI-option-based engine
settings) from the match runner.

**Prerequisite for:** US-014 (benchmark script needs to configure Stockfish Elo/skill).

**Acceptance Criteria:**
- [ ] `UciEngine` in `chesslib/src/uci_engine.rs` gains a public async method:
  `pub async fn set_option(&mut self, name: &str, value: &str) -> Result<()>`
- [ ] Method sends `setoption name <name> value <value>\n` to the engine stdin
- [ ] No response is expected (UCI spec: setoption has no reply); method returns after flush
- [ ] Existing `Engine` trait is NOT modified — `set_option` is a concrete method on `UciEngine`
  only (not part of the trait), since not all engines support UCI options
- [ ] Mock engine in tests silently ignores `setoption` lines (already does via default case)
- [ ] Unit test: sends `setoption name Skill Level value 10` to mock engine without error
- [ ] `cargo test --workspace` passes

### US-014: Benchmark script for new engine versions

**Description:** As a developer, I want a shell script that automates benchmarking a new engine
version so that I get consistent, reproducible results every time I cut a new version.

**Acceptance Criteria:**
- [ ] Script at `scripts/benchmark-version.sh` accepts arguments: `--version`, `--engine`,
  `--prev-engine` (optional), `--stockfish` (path), `--games` (default 100), `--output-dir`
- [ ] Script runs: new engine vs previous engine (if provided) using SPRT (`chess-runner sprt`)
- [ ] Script runs: new engine vs Stockfish at four skill levels:
  - 1500 Elo (`UCI_LimitStrength true`, `UCI_Elo 1500`)
  - 2000 Elo (`UCI_Elo 2000`)
  - 2500 Elo (`UCI_Elo 2500`)
  - Max (no limit)
  - Time control: 10s + 0.1s inc (bullet, consistent with current default direction)
- [ ] Stockfish skill is configured via `UciEngine::set_option` (US-017): sends
  `setoption name UCI_LimitStrength value true` + `setoption name UCI_Elo value <N>` before each
  match; for max strength, sends `setoption name UCI_LimitStrength value false`
- [ ] Script appends all results to a single CSV at `--output-dir/results.csv`
- [ ] Script prints a summary to stdout when done
- [ ] Script is executable (`chmod +x`) and includes a usage comment at the top

### US-015: `version-report` subcommand in chess-runner

**Description:** As a developer, I want a `chess-runner version-report` command that generates a
rich markdown report for a specific engine version so that I have a self-contained snapshot of its
performance.

**Acceptance Criteria:**
- [ ] New subcommand `chess-runner version-report` with arguments: `--version`, `--input` (CSV
  path), `--output` (markdown output path), optional `--engine-name`
- [ ] Reads the CSV and filters rows where `engine1_name` matches the given version/engine name
- [ ] Generated markdown includes:
  - Header with version name and generation date
  - Summary table: opponent, games, wins, draws, losses, win%, SPRT result (if present in CSV)
  - Overall win rate across all opponents
  - Best and worst matchup
  - A `## Notes` section at the end (empty, to be filled manually)
- [ ] If `--output` points to `chess-engine/versions/vX.Y/report.md`, the file is created or
  overwritten
- [ ] Handles missing/empty CSV gracefully
- [ ] `cargo test --workspace` passes

### US-016: Generate v0.1 benchmark report

**Description:** As a developer, I want to run the benchmark script for v0.1 and commit the
resulting report so that we have a baseline for measuring future improvements.

**Acceptance Criteria:**
- [ ] `scripts/benchmark-version.sh` runs successfully for v0.1 against Stockfish at all four
  levels (no previous engine version exists for v0.1)
- [ ] `chess-runner version-report` generates `chess-engine/versions/v0.1/report.md`
- [ ] Report is committed to the repository
- [ ] Stockfish must be available at `engines/stockfish` (per US-001 from prior PRD)

## Functional Requirements

- FR-1: `chess-runner match --verbose` prints board + moves; without flag, only per-game result
  line + running score is printed
- FR-2: Engine struct is `WiggumEngine`, UCI name is `"Wiggum Engine v0.1"`, name in CSV logs
  reflects this
- FR-3: `chess-engine/versions/` contains one subfolder per engine version, each with `CHANGES.md`
  and `report.md`
- FR-4: `chess-runner sprt` implements LLR-based stopping rule with configurable H0/H1 and
  alpha/beta parameters
- FR-5: LLR is recomputed and printed after each game during SPRT
- FR-6: `scripts/benchmark-version.sh` runs SPRT vs previous engine and fixed-game matches vs
  Stockfish at four skill levels
- FR-7: Stockfish skill levels configured via `UCI_LimitStrength` + `UCI_Elo` UCI options
- FR-8: `chess-runner version-report` generates a markdown file from CSV data with summary table,
  overall stats, and an empty Notes section
- FR-9: The current default time control (60s + 0 inc, bullet) is validated as intentional and
  documented in the versions README

## Non-Goals

- No alpha-beta pruning or search improvements in this PRD (those are future versions)
- No automatic Elo calculation or rating list (SPRT only measures relative improvement)
- No GUI or web visualization
- No multi-threaded SPRT game dispatch
- No opening book diversification for benchmark games (all games start from startpos)
- No automatic version tagging in git

## Technical Considerations

- SPRT LLR formula: use the trinomial (W/D/L) model. Score `s = (W + 0.5*D) / N`, then
  `elo = -400 * log10(1/s - 1)`. LLR approximation:
  `LLR = N * (elo_diff * ln10/400)` — for the full formula, see fishtest source or use the
  standard Elo-based LLR: `LLR = W*ln(p1_w/p0_w) + D*ln(p1_d/p0_d) + L*ln(p1_l/p0_l)` where
  `p0_*` and `p1_*` are the expected W/D/L probabilities under H0 and H1
- The CSV schema from US-008 does not include SPRT result; the `sprt` subcommand should add a
  `sprt_result` column when writing, or write to a separate CSV
- `version-report` must handle CSVs with and without the `sprt_result` column
- Stockfish setoption must be sent after `ucinewgame` and before `go` — verify this works with the
  existing `UciEngine` in chesslib, which may need a `set_option` method
- The benchmark script uses `chess-runner sprt` for the vs-previous match and plain
  `chess-runner match` for the vs-Stockfish matches (fixed games, not SPRT, since Stockfish's Elo
  is fixed and we just want a win rate)

## Time Control Validation

The current default (`--time 60000 --inc 0`) is **bullet** (1 minute, no increment). This is
appropriate for engine testing: fast iteration, many games possible. Standard engine testing
communities (CCRL, fishtest) use bullet to rapid time controls.

For benchmark purposes, `scripts/benchmark-version.sh` will use **10s + 0.1s inc** (fast bullet),
which allows ~100 games per opponent in reasonable time while still being representative.

## Success Metrics

- Running `chess-runner match --engine1 X --engine2 Y --games 10` without `--verbose` produces
  exactly 10 result lines + 1 summary block
- `WiggumEngine v0.1` appears in CSV logs and UCI handshake
- `chess-runner sprt` terminates in under 200 games for a 50-Elo difference between engines
- `chess-engine/versions/v0.1/report.md` exists and contains correct stats after benchmark run
- `cargo test --workspace` passes after all changes

## Resolved Questions

- **`UciEngine.set_option`:** Não existe na implementação atual. US-017 cobre a implementação.
  US-014 depende de US-017.
- **CSV separado para SPRT:** Confirmado — SPRT escreve em `sprt_results.csv` separado para evitar
  incompatibilidade de colunas com o CSV de matches regulares. O subcomando `version-report`
  (US-015) aceita ambos os arquivos via flags distintas.
