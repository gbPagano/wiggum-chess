# PRD: Iterative Deepening Timed Search

## Introduction

Add iterative deepening to the engine so it searches progressively deeper plies until its allotted time expires, instead of always searching a fixed depth. The feature must support both fixed movetime and clock-based UCI time controls (`wtime`, `btime`, `winc`, `binc`, `movetime`), allocate search time dynamically, and always return the best move found so far before the deadline.

This solves two current limitations: the engine cannot scale search depth up or down based on available time, and it cannot make practical use of increments or remaining clock during timed games. The first implementation should add iterative deepening and time management on top of the existing search, while leaving future search optimizations such as alpha-beta and move ordering out of scope.

## Goals

- Allow the engine to search depth 1, then 2, then 3, and so on until the time budget is exhausted.
- Support both fixed movetime searches and clock/increment-based searches via UCI time control parameters.
- Ensure the engine returns a legal move before the deadline in all timed search modes.
- Make the engine search deeper when given more available time.
- Track and preserve the best evaluated move found so far across completed and in-progress depths.
- Allow future time-allocation and search-strength improvements without redesigning the interface introduced here.

## User Stories

### US-001: Parse timed UCI search parameters
**Description:** As a developer, I want the engine to accept UCI timing parameters so that timed search can be driven by either a fixed movetime or the remaining game clock.

**Acceptance Criteria:**
- [ ] The engine accepts `go movetime <ms>`.
- [ ] The engine accepts `go wtime <ms> btime <ms> winc <ms> binc <ms>`.
- [ ] The engine selects the active side’s clock and increment based on side to move.
- [ ] Unsupported or absent timing parameters fall back to the existing non-timed search behavior defined in the PRD.
- [ ] `cargo build --workspace` passes.

### US-002: Compute a per-move time budget
**Description:** As a developer, I want the engine to convert UCI timing inputs into a per-move budget so that search depth can scale to the time available.

**Acceptance Criteria:**
- [ ] A time-budget calculation exists for fixed movetime searches.
- [ ] A time-budget calculation exists for clock-plus-increment searches.
- [ ] For clock-plus-increment mode, the first implementation uses a simple baseline heuristic: `target_time = remaining_time / 20 + increment / 2`.
- [ ] The final allocated budget is capped below the remaining clock by a safety margin so the engine returns before the hard deadline.
- [ ] The heuristic is documented near the time-management code.
- [ ] `cargo build --workspace` passes.

### US-003: Search with iterative deepening
**Description:** As a chess engine, I want to search one depth at a time and keep deepening while time remains so that I can use as much of the allocated time as possible.

**Acceptance Criteria:**
- [ ] Search starts at depth 1 and increases by 1 ply per completed iteration.
- [ ] Each completed depth produces a candidate best move and score.
- [ ] The deepening loop stops when the allotted time is exhausted or when another explicit search limit is reached.
- [ ] The implementation reuses the current negamax search as the underlying search algorithm.
- [ ] `cargo build --workspace` passes.

### US-004: Stop safely when time expires
**Description:** As a chess engine, I want timeout checks during search so that I can stop promptly and still return the best move found so far.

**Acceptance Criteria:**
- [ ] Search includes timeout checks often enough to stop before exceeding the configured deadline.
- [ ] If time expires during a deeper search, the engine returns the best evaluated move found so far.
- [ ] If at least one depth completed, the engine can return the strongest known move from completed or partially explored deeper search state, as defined by the chosen policy.
- [ ] If no full depth completed, the engine still returns a legal move.
- [ ] `cargo build --workspace` passes.

### US-005: Define best-so-far move selection policy ✅
**Description:** As a developer, I want a clear rule for choosing the move returned on timeout so that iterative deepening behavior is deterministic and testable.

**Acceptance Criteria:**
- [x] The implementation defines what “best evaluated move so far” means during an incomplete depth.
- [x] The timeout policy prefers the best move from the last fully completed depth by default.
- [x] A partial-depth move may replace the last fully completed move only if the root move being explored has finished evaluation and its score exceeds the completed-depth best by at least 30 centipawns.
- [x] The initial implementation does not apply any additional deeper-analysis bonus beyond that 30 centipawn override threshold.
- [x] The chosen policy is documented in code comments or design notes near the search entry point.
- [x] `cargo build --workspace` passes.

### US-006: Validate timed search behavior ✅
**Description:** As a maintainer, I want tests around iterative deepening and time handling so that the engine remains correct while timed search is added.

**Acceptance Criteria:**
- [x] Tests cover fixed movetime search returning a legal move.
- [x] Tests cover clock/increment search returning a legal move.
- [x] Tests cover timeout during iterative deepening returning the configured best-so-far move.
- [x] Existing workspace tests continue to pass using `cargo test --workspace -- --skip gen_files::magics::name`.
- [x] Test coverage does not rely on flaky wall-clock assertions tighter than the engine can reliably satisfy.

## Functional Requirements

- FR-1: The engine must support iterative deepening by running depth-limited searches starting at depth 1 and increasing by one ply per iteration.
- FR-2: The engine must support UCI `go movetime <ms>` as a fixed per-move time limit.
- FR-3: The engine must support UCI clock-based search inputs `wtime`, `btime`, `winc`, and `binc`.
- FR-4: For clock-based searches, the engine must use the side to move to select the relevant remaining clock and increment.
- FR-5: The engine must convert timing inputs into an internal search deadline or budget before search begins.
- FR-6: For clock-based searches, the first-pass allocation heuristic must use `remaining_time / 20 + increment / 2` before applying a safety cap.
- FR-7: The engine must reserve a safety margin so it attempts to return before the external time limit is exceeded.
- FR-8: The iterative deepening loop must stop when the internal deadline is reached or when no deeper search is permitted by the configured limits.
- FR-9: The underlying search for each depth must remain the current negamax implementation in this phase of work.
- FR-10: The search must perform timeout checks during recursive exploration, not only between completed depths.
- FR-11: The engine must retain a legal fallback move at all times during timed search.
- FR-12: The engine must retain the best move and evaluation from each completed depth.
- FR-13: The engine must define and implement a deterministic policy for whether partial results from an interrupted deeper depth can replace the last completed depth’s move.
- FR-14: The default timeout policy must return the best move from the last fully completed depth.
- FR-15: A partial-depth move may replace the completed-depth move only when its root evaluation finished before timeout and its score exceeds the completed-depth best by at least 30 centipawns.
- FR-16: The initial implementation must not apply any additional deeper-analysis bonus beyond the 30 centipawn override threshold.
- FR-17: When time expires before any depth fully completes, the engine must still return a legal move from the current position.
- FR-18: The timed search interface must be compatible with match play through the existing UCI command handling.
- FR-19: The implementation must preserve current non-timed search behavior when timed parameters are not supplied.
- FR-20: The codebase must include tests for timed search input parsing, deadline selection, and timeout-safe move return behavior.
- FR-21: The timed iterative deepening loop must enforce an explicit maximum depth guard of 64 plies even when time remains.

## Non-Goals

- Adding alpha-beta pruning in this feature.
- Adding move ordering, transposition tables, quiescence search, aspiration windows, or other search-strength optimizations.
- Reworking the evaluation function beyond what is required to support reporting the best-so-far move.
- Implementing advanced tournament-grade time management heuristics beyond a documented first-pass allocation strategy.
- Guaranteeing the engine uses the theoretically optimal amount of time in every game state.

## Design Considerations

- The return-on-timeout rule should be easy to explain and verify. The chosen default is the common engine-friendly policy of trusting the last fully completed iteration, while allowing a deeper partial root result to replace it only when that result fully evaluated and clearly beats the previous best.
- If a deeper-analysis bonus is used, keep it as a small fixed value so the behavior stays transparent and testable.
- The first implementation should favor a simple policy that can be tested deterministically over a sophisticated but opaque scoring adjustment.
- UCI `info depth ... score ...` output is out of scope for this feature and can be added later.
- Any public or user-visible search info output should remain compatible with the existing UCI interaction style.

## Technical Considerations

- Current search is implemented as recursive negamax in `chess-engine/src/search.rs`; iterative deepening should wrap this existing search rather than replacing it.
- UCI command parsing in `chess-engine/src/main.rs` will need to recognize and forward timing parameters.
- The design will likely require a search context object carrying deadline, timeout state, and current best-so-far move.
- Timeout handling must avoid partial-state corruption and must unwind recursion safely.
- Tests should use tolerances and mocked or controlled timing boundaries where possible to avoid flaky failures.
- The timed search should include an explicit maximum depth guard even in timed mode to prevent pathological runaway search.
- The implementation should leave room for future search improvements to reuse the same timed-search entry point.

## Success Metrics

- The engine always returns a legal move before the deadline in timed search modes.
- The engine searches more plies when given more time on the same position.
- The engine accepts and uses UCI fixed movetime and clock/increment inputs during match play.
- Workspace build and tests pass, excluding the known intentional failing scratchpad test: `cargo test --workspace -- --skip gen_files::magics::name`.

## Open Questions

- None for the initial implementation.
