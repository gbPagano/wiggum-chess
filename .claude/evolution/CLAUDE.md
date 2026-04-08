# Evolution Worker Guidance

This file defines the behavior, constraints, and lifecycle of a worker agent operating inside a Wiggum engine evolution iteration. It is referenced by `scripts/evolution-loop.sh` and the orchestration script expects the worker to honor these contracts.

## Scope

The worker is limited to **engine-evolution-relevant changes only**: code, configuration, benchmarks, build scripts, and tests that directly relate to the chess engine's performance or correctness. The worker must NOT touch unrelated files, CI plumbing not owned by this repo, user-facing documentation, or dependency versions outside the engine itself.

## Phase Lifecycle

Every iteration proceeds through the following phases in order. The orchestration script creates the iteration directory and initial `iteration.json` before invoking any phase.

### 1. Propose

- Read prior session artifacts (previous iterations, session summary, accepted baselines) and the current `iteration.json`.
- Formulate a single focused hypothesis about what change could improve the engine.
- Write the hypothesis to `hypothesis.md` in the iteration directory.
- Update `iteration.json` with hypothesis metadata (description, proposed metrics to track).
- If no valid hypothesis can be formed (e.g., prior iterations exhausted viable ideas), write `hypothesis.md` explaining why and return a stop signal by writing `"no_hypothesis"` as the hypothesis state.

### 2. Implement

- Read `hypothesis.md` to understand the proposed change.
- Apply the candidate changes to the engine codebase within the isolated workspace provided by the orchestration script.
- Write `implementation.md` summarizing the changes and listing every modified file.
- Update `iteration.json` with implementation status (`implemented` or `failed`) and the list of changed files.

### 3. Validate (Correctness Gate)

- Run configured correctness checks (e.g., `bash -n scripts/evolution-loop.sh`, `cargo build --workspace`, `cargo test --workspace -- --skip gen_files::magics::name`).
- Record which checks ran and their pass/fail results in `iteration.json`.
- If any correctness check fails, do NOT proceed to benchmark. Mark the iteration as a correctness failure.

### 4. Benchmark

- Run the benchmark suite against the candidate changes using the configured benchmark policy (see US-011 benchmark policy contract).
- Start with screening evidence, and require stronger confirmation evidence before promotion when the screening signal is weak, early, or ambiguous.
- Write `benchmark.md` with: benchmark settings, SPRT results, game count, and summary metrics.
- Update `iteration.json` with benchmark status plus the policy fields `benchmark.policyStage` and `benchmark.sufficientForPromotion`, along with the minimum required metrics (`gamesCompleted`, `sprtResult`, `scorePerGame`).

### 5. Decide

- Read `implementation.md` and `benchmark.md`.
- Determine the final outcome: `accepted`, `rejected`, `failed`, or `inconclusive`.
- Write `decision.md` with the outcome and reasoning.
- Update `iteration.json` with the final state.

### 6. Persist

- The orchestration script handles persisting accepted changes or discarding rejected ones.
- The worker must NOT persist changes to the main branch directly; that is the role of the orchestration script after the `accepted` decision.

### 7. Repeat

- The orchestration script increments the iteration counter and returns to phase 1 unless a stop condition has been reached.

## Outcome Behaviors

| Outcome        | Behavior |
| --- | --- |
| `accepted`     | Changes are merged to the accepted baseline. Version is bumped per the versioning policy. Next iteration starts from the new baseline. |
| `rejected`     | Changes are discarded. Iteration artifacts are preserved for audit. Next iteration starts from the unchanged baseline. |
| `failed`       | Correctness gate or benchmark infrastructure failed. Changes are discarded. Iteration artifacts preserved. Next iteration starts from the unchanged baseline. |
| `inconclusive` | Evidence was insufficient for promotion. Changes may be refined in a future iteration. Baseline unchanged. |
| `no_hypothesis`| No further improvement ideas remain. Session terminates early. |

The canonical versioning policy lives in `tasks/prd-wiggum-evolution-loop.md`. Accepted outcomes must sync the workspace crate versions with the promoted `chess-engine/versions/<tag>/` directory, while non-winning outcomes must leave both unchanged.

## Iteration State Machine

The canonical state-machine contract lives in `tasks/prd-wiggum-evolution-loop.md` and is referenced by the orchestration script through `STATE_MACHINE_REFERENCE`.

Use these `iteration.json.state` values only:

- In-progress: `initialized`, `proposing`, `proposed`, `implementing`, `implemented`, `validating`, `benchmarked`, `deciding`
- Final: `accepted`, `rejected`, `inconclusive`, `failed`

Valid transitions:

```
initialized -> proposing -> proposed -> implementing -> implemented -> validating
validating -> implemented
validating -> failed
implemented -> benchmarked
benchmarked -> deciding -> [accepted|rejected|inconclusive]
deciding -> failed
```

The orchestration script and decision skill enforce these transitions. Invalid transitions are treated as hard orchestration errors, not automatic recovery opportunities.

`iteration.json.state` is the authoritative machine-readable value, and `iteration.json.stateMachine.currentPhase` should mirror it for easier inspection.

## File Layout

Each iteration directory (`iterations/<N>/`) contains:

| File | Owner | Purpose |
| --- | --- | --- |
| `iteration.json` | Script + Skills | State, metadata, and artifact paths |
| `hypothesis.md` | Propose skill | Proposed improvement |
| `implementation.md` | Implement skill | Change summary and file list |
| `benchmark.md` | Benchmark skill | Benchmark results |
| `decision.md` | Decide skill | Final outcome and reasoning |

## References

- Orchestration script: `scripts/evolution-loop.sh`
- Project PRD: `tasks/prd.json`
- Session artifacts: `summary.md`, `session.env` at session root
- Iteration state machine: defined above, enforced by orchestration script and decide skill
