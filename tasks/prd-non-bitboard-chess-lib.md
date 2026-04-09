# PRD: Non-Bitboard Chess Library

## Introduction

Add a new workspace crate that implements a chess library without using bitboards so it can be compared directly with `chesslib`. The goal is to provide a clean, readable, matrix-based reference implementation focused on correctness and perft comparability rather than maximum speed.

This crate will use an 8x8 board representation (`[[Option<Piece>; 8]; 8]`) and will support legal move generation plus perft. It is intended to act as a baseline for correctness and performance comparisons against the existing bitboard-based `chesslib`.

## Goals

- Add a new Rust crate to the workspace for a chess implementation that does not use bitboards.
- Keep the implementation clear and easy to read, even when that costs performance.
- Support board representation, move generation, move application, and perft.
- Reuse the same core perft positions and expectations used to validate `chesslib`, adapted to the new crate.
- Cover standard chess rules required for legal move generation, including check, castling, en passant, and promotion.
- Make it possible to compare correctness and runtime characteristics against `chesslib`.

## User Stories

### US-001: Add a new workspace crate
**Description:** As a developer, I want a dedicated crate for the non-bitboard implementation so that I can build and test it independently inside the existing workspace.

**Acceptance Criteria:**
- [ ] A new library crate is added to the workspace members in the root `Cargo.toml`.
- [ ] The crate builds with `cargo build --workspace`.
- [ ] The crate has a clear name that distinguishes it from `chesslib`.
- [ ] The crate exposes a public API for board setup, move generation, and perft.

### US-002: Represent the board without bitboards
**Description:** As a developer, I want the board stored as an 8x8 matrix so that the implementation is straightforward to inspect and reason about.

**Acceptance Criteria:**
- [ ] The board representation uses an 8x8 matrix structure such as `[[Option<Piece>; 8]; 8]`.
- [ ] The implementation does not use bitboards internally.
- [ ] Piece color, piece kind, side to move, castling rights, en passant state, and game state required for move legality are represented explicitly.
- [ ] The code paths for reading and updating board state are covered by tests.

### US-003: Generate legal chess moves
**Description:** As a developer, I want the crate to generate legal moves so that I can validate correctness against known chess rules and perft results.

**Acceptance Criteria:**
- [x] The crate generates legal moves for all standard piece types.
- [x] Illegal moves that leave the king in check are excluded.
- [x] Castling is generated only when all relevant legal conditions are satisfied.
- [x] En passant is generated only when legal.
- [x] Promotions are generated correctly for all required promotion piece types.
- [x] Tests cover check, pinned pieces, castling, en passant, and promotion scenarios.

### US-004: Apply moves and derive child positions
**Description:** As a developer, I want moves to be applied to produce new board states so that perft can traverse the move tree correctly.

**Acceptance Criteria:**
- [x] A legal move can be applied to a board to produce the resulting board state.
- [x] Move application updates side to move, castling rights, en passant state, captures, and promotions correctly.
- [x] Special moves (castling, en passant, promotion) update the board correctly.
- [x] Tests verify resulting board state for representative normal and special moves.

### US-005: Run perft against known reference positions
**Description:** As a developer, I want perft support so that I can compare node counts and basic runtime behavior against `chesslib`.

**Acceptance Criteria:**
- [x] The crate exposes a perft function that counts leaf nodes for a given depth.
- [x] The implementation passes perft tests for the starting position and additional known reference positions.
- [x] The perft test cases match the same core scenarios already used by `chesslib`, adapted to the new crate API.
- [x] At least one test covers a position with castling rights.
- [x] At least one test covers a position with en passant availability.
- [x] At least one test covers a position with promotion opportunities.

### US-006: Make comparison with `chesslib` straightforward
**Description:** As a developer, I want a simple way to compare outputs with `chesslib` so that I can validate correctness and study tradeoffs between the two implementations.

**Acceptance Criteria:**
- [x] The new crate documents or exposes an obvious entry point for running perft.
- [x] The same perft depths and positions used for `chesslib` tests can be executed against the new crate.
- [x] The crate can be tested independently without modifying `chesslib` internals.
- [x] Workspace tests pass using the existing project test workflow, accounting for the known skipped scratchpad test.

## Functional Requirements

- FR-1: The system must add a new library crate to the Cargo workspace.
- FR-2: The new crate must implement chess logic without using bitboards internally.
- FR-3: The board representation must use an 8x8 matrix structure storing optional pieces per square.
- FR-4: The crate must represent enough game state to determine legal moves, including side to move, castling rights, en passant target, and king safety.
- FR-5: The crate must generate legal moves for pawns, knights, bishops, rooks, queens, and kings.
- FR-6: The crate must support move application that returns or constructs the resulting board state after a legal move.
- FR-7: The crate must handle castling, en passant, and promotion correctly during both move generation and move application.
- FR-8: The crate must reject moves that leave the moving side's king in check.
- FR-9: The crate must expose a perft function for counting nodes to a requested depth.
- FR-10: The crate must include automated tests for normal move generation and special-rule edge cases.
- FR-11: The crate must include perft tests using the same core reference scenarios used by `chesslib`, adapted as needed.
- FR-12: The crate must build and test as part of the workspace.

## Non-Goals

- This work will not implement a search engine or UCI binary.
- This work will not aim to outperform `chesslib`.
- This work will not duplicate the full architecture of `chesslib`.
- This work will not add bitboard compatibility layers.
- This work will not introduce benchmarking infrastructure beyond what is needed to run comparable perft checks.

## Design Considerations

- Favor explicit data structures and readable control flow over highly optimized representations.
- Keep naming and concepts familiar enough that developers can compare behavior with `chesslib`, even though the API may remain independent.
- Separate board representation, move generation, and perft logic clearly enough that the crate can serve as a reference implementation.

## Technical Considerations

- The implementation should use a matrix-based board representation, likely `[[Option<Piece>; 8]; 8]`.
- The new crate should be independent from `chesslib` internals, but it should reuse the same known perft expectations where practical.
- The existing workspace test guidance still applies: full workspace test runs must skip `gen_files::magics::name`.
- Correctness matters more than speed, but the API should make repeated perft runs practical for comparison.
- Tests should target the chess rules most likely to diverge from `chesslib`: check detection, castling legality, en passant legality, and promotions.

## Success Metrics

- The new crate builds cleanly in the workspace.
- The new crate passes legal-move and special-rule tests.
- The new crate matches expected perft node counts on the selected reference positions.
- Developers can run the same comparison-oriented perft scenarios against both `chesslib` and the new crate.
- The implementation is readable enough to serve as a correctness/reference baseline.

## Open Questions

- What crate name should be used for the non-bitboard implementation?
- Should the crate include FEN parsing in the first version, or should tests construct positions directly?
- Should a small CLI hook be added later in `chess-runner` for direct side-by-side perft comparison, or should comparison remain test-only for now?
