# PRD: PGN Support

## Introduction

Add Portable Game Notation (PGN) parsing to `chesslib` so any crate in the workspace can load real-world games, replay moves to an arbitrary position, and produce a `Board` state ready for engine use. `chess-runner` will expose this via two new subcommands: one that prints the FEN at a given move, and one that starts an engine match directly from that position.

The primary motivation is testing: importing established game histories lets developers reproduce mid-game and endgame positions reproducibly, validate move logic against known sequences, and run perft on positions that would be tedious to construct by hand.

## Goals

- Parse single-game PGN text into an ordered list of moves within `chesslib`
- Replay any prefix of those moves and return the resulting `Board`
- Expose a `chess-runner pgn-fen` subcommand that prints the FEN at move N
- Expose a `chess-runner pgn-match` subcommand that starts a two-engine match from that position
- Validate correctness: every parsed move must produce a FEN that matches a known-good reference or passes perft

## User Stories

### US-001: PGN parser in `chesslib`
**Description:** As a developer, I want a `chesslib` function that parses a PGN string into a sequence of `ChessMove` values so that any crate can replay games without duplicating parsing logic.

**Acceptance Criteria:**
- [ ] `chesslib::pgn::parse(pgn: &str) -> Result<Vec<ChessMove>, PgnError>` exists and is public
- [ ] Parses standard SAN move tokens (e.g. `e4`, `Nf3`, `O-O`, `exd5`, `Qxh7+`, `Bxf7#`)
- [ ] Strips PGN headers (`[Tag "Value"]` lines) and move numbers before parsing tokens
- [ ] Strips inline comments (`{ ... }`) and annotation glyphs (`!`, `?`, `!!`, `??`, `!?`, `?!`) silently
- [ ] Returns `PgnError` with a descriptive message on invalid/ambiguous SAN
- [ ] `cargo test --workspace` passes

### US-002: Replay moves to a target position
**Description:** As a developer, I want to replay N moves from a parsed PGN so that I can obtain the `Board` at any point in the game.

**Acceptance Criteria:**
- [ ] `chesslib::pgn::replay(moves: &[ChessMove], up_to: usize) -> Result<Board, PgnError>` exists and is public
- [ ] `up_to = 0` returns the starting position; `up_to = moves.len()` returns the final position
- [ ] Returns `PgnError` if any move in the prefix is illegal on the board at that point
- [ ] `cargo test --workspace` passes

### US-003: `chess-runner pgn-fen` subcommand
**Description:** As a developer, I want to run `chess-runner pgn-fen --file game.pgn --move 20` and get the FEN string printed to stdout so I can pipe it into other tools or inspect the position.

**Acceptance Criteria:**
- [ ] `chess-runner pgn-fen --file <path> --move <N>` prints a valid FEN string to stdout
- [ ] `--move 0` prints the starting FEN
- [ ] `--move` defaults to the final position if omitted
- [ ] Exits with a non-zero code and an error message on parse failure or move out of range
- [ ] `cargo test --workspace` passes

### US-004: `chess-runner pgn-match` subcommand
**Description:** As a developer, I want to run a two-engine match starting from a position loaded from a PGN file so that I can test engine behavior on specific mid-game or endgame scenarios.

**Acceptance Criteria:**
- [ ] `chess-runner pgn-match --file <path> --move <N> --engine1 <bin> --engine2 <bin> [--games <n>] [--time <ms>] [--csv <path>]` works end-to-end
- [ ] Passes the reconstructed FEN to both engines via `position fen ...` in the UCI protocol
- [ ] Reuses all existing `Match` / `MatchArgs` infrastructure; only the start FEN differs
- [ ] Match result is printed and optionally appended to CSV in the same format as `chess-runner match`
- [ ] `cargo test --workspace` passes

### US-005: Correctness test suite
**Description:** As a developer, I want automated tests that verify PGN parsing against known-correct positions so I can trust the parser.

**Acceptance Criteria:**
- [ ] At least one full real-world PGN game (e.g. a famous historical game) is embedded in the test suite
- [ ] For each move in that game, the produced FEN is compared against a pre-computed reference list
- [ ] At least one test runs perft(1) on the final position and checks the node count
- [ ] At least one test that exercises castling SAN (`O-O`, `O-O-O`)
- [ ] At least one test that exercises promotion SAN (e.g. `e8=Q`)
- [ ] `cargo test --workspace` passes

## Functional Requirements

- FR-1: `chesslib` must expose a `pgn` module (behind no feature flag) containing `parse()` and `replay()`.
- FR-2: SAN resolution must handle: pawn pushes, pawn captures (including en passant), piece moves, captures, kingside/queenside castling, promotions, and move decorators (`+`, `#`).
- FR-3: When a SAN token is ambiguous (two pieces of the same type can reach the destination), the parser must use the file/rank disambiguation characters already present in the token (e.g. `Rdf8`, `R1a3`).
- FR-4: `chess-runner` must add `pgn-fen` and `pgn-match` as first-class `clap` subcommands alongside the existing ones.
- FR-5: `pgn-match` must accept the same `--time`, `--inc`, `--games`, `--timeout`, and `--csv` flags as `match`; when `--games > 1`, engines must alternate colors each game (same behavior as `match`).
- FR-7: Both `pgn-fen` and `pgn-match` must accept an optional `--game <N>` flag (1-based index, default 1) to select which game to load from a multi-game PGN file.
- FR-6: All errors from the PGN layer must surface as human-readable messages (not panics).

## Non-Goals

- Multi-game PGN files (only the first game in the file will be parsed; additional games are ignored or an error is returned — TBD implementation choice)
- Recursive alternative variations (`(e4 e5 (d5 ...))`)
- Writing/exporting PGN
- PGN validation beyond what is needed for move replay (e.g. result tag checking)
- Time-control annotations (`{[%clk ...]}`), which are stripped silently like other comments
- A GUI or interactive board display

## Technical Considerations

- SAN resolution requires a legal move generator: resolve each SAN token by generating all legal moves for the current `Board` and filtering by destination square, piece type, and disambiguation — lean on the existing `MoveGen` iterator.
- `ChessMove::from_uci` already exists; SAN→`ChessMove` is a new code path. Keep them separate; do not convert SAN→UCI string as an intermediate step.
- The `pgn` module should live at `chesslib/src/pgn.rs` and be declared in `chesslib/src/lib.rs`.
- `pgn-match` should reuse `MatchArgs` parsing logic and `Match::new()` from `chesslib::match_runner`; pass the reconstructed FEN as `start_fen` — the field already exists on `MatchArgs`.
- `PgnError` must implement `std::error::Error` and `From<PgnError> for anyhow::Error` (via `thiserror` or a manual impl) so it composes with `?` throughout the workspace, consistent with how other errors are handled.
- Multi-game PGN parsing: `chesslib::pgn::parse_nth(pgn: &str, game_index: usize) -> Result<Vec<ChessMove>, PgnError>` selects game by 0-based index internally; the CLI flag is 1-based and subtracts 1 before calling. `parse()` is a convenience wrapper for `parse_nth(pgn, 0)`.

## Success Metrics

- All moves in at least one complete real-world PGN game parse and reproduce the correct FEN at every ply (verified against a reference list).
- perft(1) on the final position of that game returns the expected node count.
- `chess-runner pgn-fen` and `chess-runner pgn-match` work end-to-end in a manual smoke test.
- Zero panics on malformed PGN input — all errors are returned, not unwrapped.

## Open Questions

None — all open questions resolved during PRD review.
