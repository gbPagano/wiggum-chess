# PRD: Wiggum Engine Evolution Loop

## 1. Introduction / Overview

Create a Ralph-like autonomous evolution harness for Wiggum Engine.

The deliverable must be runnable end to end from this repository and must include:
- an orchestration script that starts and advances the loop;
- Claude Code skills for the main worker phases;
- a dedicated worker prompt or `CLAUDE.md` for the loop agent;
- deterministic artifact storage for iteration state, benchmark evidence, and promotion decisions.

The loop must repeatedly propose one engine improvement, implement it in isolation, validate it against the current baseline, promote it only if it wins, discard it otherwise, and continue until `max-iterations` is reached or no valid next step exists.

## 2. Goals

- Provide a runnable Ralph-style harness, not only a conceptual workflow.
- Allow the user to start an unattended engine-evolution session with one command.
- Break the workflow into small Claude Code skills with explicit contracts.
- Provide dedicated worker guidance so the loop agent behaves consistently.
- Keep each iteration narrow: one primary engine-improvement hypothesis per attempt.
- Validate each candidate with at least one SPRT run of **10 or more games**.
- Prefer stronger validation than 10 games when the first result is weak or inconclusive.
- Promote only candidates that satisfy the configured acceptance policy.
- Discard rejected or failed candidates without contaminating the accepted baseline.
- Persist enough history to understand what was tried, why, and with what result.
- Stop automatically at `max-iterations` or another explicit stop condition.

## 3. Required Layout and Contracts

### Required file layout

- Orchestration script: `scripts/evolution-loop.sh`
- Worker guidance: `.claude/evolution/CLAUDE.md`
- Skills:
  - `.claude/skills/evolution-propose/`
  - `.claude/skills/evolution-implement/`
  - `.claude/skills/evolution-benchmark/`
  - `.claude/skills/evolution-decide/`
- Session artifacts root: `tasks/evolution-runs/<session-id>/`
- Session id format: UTC timestamp `YYYYMMDDTHHMMSSZ` generated once at session start and reused for all session artifacts.
- Session root contents:
  - `summary.md` — top-level session summary placeholder that later stories will finalize
  - `session.env` — selected session inputs such as `baseline_version` and `max_iterations`

This keeps each run in a deterministic directory once the session starts while still making session ids easy to sort and inspect manually.

### Session root example

```text
tasks/evolution-runs/20260406T185545Z/
├── session.env
└── summary.md
```


### Iteration contract

For iteration `N`, the script must create `tasks/evolution-runs/<session-id>/iterations/<N>/` with at least:
- `iteration.json` — machine-readable iteration state
- `hypothesis.md` — selected idea and rationale
- `implementation.md` — implementation result and changed files summary
- `correctness/results.md` — configured correctness checks and benchmark eligibility
- `benchmark.md` — benchmark settings and results
- `decision.md` — final outcome and reason

The initial `iteration.json` written by the script must include at least:
- `iteration` — numeric iteration number
- `baselineVersion` — selected baseline version for this iteration
- `state` — initial state set to `initialized`
- `correctness` — gate status with `passed`, `benchmarkEligible`, and per-check results
- `artifacts` — paths for `iterationJson`, `hypothesis`, `implementation`, `correctness`, `benchmark`, and `decision`

### Iteration artifact example

```text
tasks/evolution-runs/20260406T185545Z/
└── iterations/
    └── 1/
        ├── benchmark.md
        ├── correctness/
        │   └── results.md
        ├── decision.md
        ├── hypothesis.md
        ├── implementation.md
        └── iteration.json
```

### Skill handoff contract

- The script creates the iteration directory and initial `iteration.json`.
- `evolution-propose` reads prior session artifacts and writes `hypothesis.md` plus updated `iteration.json`.
- `evolution-implement` reads `hypothesis.md`, applies the candidate in isolation, and writes `implementation.md` plus updated `iteration.json`.
- The orchestration script runs the configured correctness gate after implementation, writes `correctness/results.md`, and updates `iteration.json` with check results and benchmark eligibility.
- `evolution-benchmark` reads current iteration state, runs validation only when `correctness.benchmarkEligible` is `true`, and writes `benchmark.md` plus updated `iteration.json`.
- `evolution-decide` reads implementation and benchmark artifacts, treats correctness-gate failure as a `failed` outcome, writes `decision.md`, and updates `iteration.json` with the final state.
- The orchestration script alone controls whether to continue, promote, discard, or stop.

### Correctness gate contract

- The orchestration flow runs configured correctness checks before any benchmark step.
- The default configured checks are `bash -n scripts/evolution-loop.sh`, `cargo build --workspace`, and `cargo test --workspace -- --skip gen_files::magics::name` from the candidate workspace.
- `iteration.json` records the correctness gate under `correctness` with `status`, `passed`, `benchmarkEligible`, and a `checks` array containing each command and its pass/fail result.
- If any configured correctness check fails, `benchmark.md` is marked skipped, `decision.md` records a `failed` outcome, and the candidate is ineligible for promotion.
- Successful correctness checks keep the iteration benchmark-eligible without promoting the candidate by themselves.

### Correctness metadata example

```json
{
  "state": "implemented",
  "correctness": {
    "status": "completed",
    "passed": true,
    "benchmarkEligible": true,
    "checks": [
      {
        "name": "bash -n scripts/evolution-loop.sh",
        "status": "passed"
      },
      {
        "name": "cargo build --workspace",
        "status": "passed"
      },
      {
        "name": "cargo test --workspace -- --skip gen_files::magics::name",
        "status": "passed"
      }
    ]
  }
}
```

A failed correctness gate must prevent benchmark execution for that iteration.

### Iteration artifact example after correctness gate

```text
tasks/evolution-runs/20260406T185545Z/
└── iterations/
    └── 1/
        ├── benchmark.md        # completed or skipped when correctness fails
        ├── correctness/
        │   └── results.md
        ├── decision.md
        ├── hypothesis.md
        ├── implementation.md
        └── iteration.json
```


### Isolated candidate workspace contract

- Every iteration creates a reversible git worktree at `tasks/evolution-runs/<session-id>/candidate-workspaces/<N>/` or the configured output directory equivalent.
- The candidate worktree is created from the latest accepted baseline reference, not from the last attempted candidate.
- `iteration.json` records the isolation metadata under `isolation`, including `type`, `path`, `branch`, `status`, and `setupError`.
- If worktree or branch setup fails, the iteration enters the `failed` state immediately, `decision.md` records the setup failure, and the accepted baseline remains unchanged.
- Candidate branches use the format `wiggum-evolution/<session-id>/iteration-<N>` so each attempt is auditable and removable without affecting the accepted baseline.
- Because implementation happens inside the isolated worktree, rejected candidates can be discarded without leaking changes into the promoted baseline.

### Isolation metadata example

```json
{
  "baselineVersion": "v0.1",
  "baselineRef": "<accepted-baseline-ref>",
  "state": "initialized",
  "isolation": {
    "type": "git-worktree",
    "path": "tasks/evolution-runs/<session-id>/candidate-workspaces/1",
    "branch": "wiggum-evolution/<session-id>/iteration-1",
    "status": "ready",
    "setupError": ""
  }
}
```

The `baselineRef` field is the authoritative source for which accepted baseline an iteration started from.

### Benchmark policy contract

- Every iteration that survives the correctness gate starts with a **screening benchmark**.
- The screening benchmark must run **at least one SPRT match with a minimum of 10 completed games**.
- A screening run may be enough to reject an obviously weaker candidate, but it is not automatically strong enough for promotion.
- **Confirmation benchmarking is required before promotion** when the screening result is weak, early, or otherwise ambiguous. This includes cases where the SPRT output is inconclusive, the Elo estimate or score-per-game signal is near neutral, the completed game count is only at the minimum threshold, or benchmark anomalies reduce confidence.
- Confirmation benchmarking should increase evidence strength by using more completed games, longer time control, or both.
- `iteration.json` must store benchmark summary data under `benchmark` with, at minimum:
  - `status`
  - `policyStage` (`screening` or `confirmation`)
  - `settings.timeControl`
  - `settings.increment`
  - `settings.gamesRequested`
  - `metrics.gamesCompleted`
  - `metrics.candidateWins`
  - `metrics.baselineWins`
  - `metrics.draws`
  - `metrics.candidateWinRate`
  - `metrics.scorePerGame`
  - `metrics.eloEstimate`
  - `metrics.sprtResult`
  - `sufficientForPromotion` (`true` only when the configured policy says the evidence is strong enough)
- The benchmark skill is responsible for writing these benchmark summary fields, and the decision skill must read them when deciding whether the iteration can be `accepted`, `rejected`, `inconclusive`, or `failed`.

## 4. User Stories

### US-001: Add orchestration script skeleton
**Description:** As a developer, I want a single script entrypoint so that I can start an evolution session with one command.

**Acceptance Criteria:**
- [ ] Create `scripts/evolution-loop.sh`.
- [ ] The script accepts `--baseline-version`, `--output-dir`, and `--max-iterations`.
- [ ] The script prints the selected inputs before any iteration starts.
- [ ] The script has a usage/help comment block at the top.
- [ ] `bash -n scripts/evolution-loop.sh` passes.

### US-002: Create session artifact root
**Description:** As a developer, I want a deterministic session directory so that each run has a stable home for artifacts.

**Acceptance Criteria:**
- [ ] The script creates `tasks/evolution-runs/<session-id>/` when a session starts.
- [ ] The session directory contains a top-level summary file placeholder.
- [ ] The session directory stores the selected baseline version and max-iterations.
- [ ] The session-id format is documented in the project.
- [ ] `bash -n scripts/evolution-loop.sh` passes.

### US-003: Create iteration artifact skeleton
**Description:** As a developer, I want each iteration to have a fixed artifact layout so that skills and the script can communicate predictably.

**Acceptance Criteria:**
- [ ] For iteration `N`, the script creates `iterations/<N>/` under the session directory.
- [ ] The iteration directory includes placeholders for `iteration.json`, `hypothesis.md`, `implementation.md`, `benchmark.md`, and `decision.md`.
- [ ] `iteration.json` includes at least: iteration number, baseline version, state, and artifact paths.
- [ ] The artifact layout is documented in the project.
- [ ] `bash -n scripts/evolution-loop.sh` passes.

### US-004: Add worker guidance file
**Description:** As a developer, I want a dedicated worker instruction file so that the loop agent behaves consistently across iterations.

**Acceptance Criteria:**
- [ ] Create `.claude/evolution/CLAUDE.md`.
- [ ] The file defines the phases: propose, implement, validate, decide, persist, repeat.
- [ ] The file limits the worker to engine-evolution-relevant changes during an iteration.
- [ ] The file defines expected behavior for accepted, rejected, failed, and inconclusive outcomes.
- [ ] The orchestration docs reference `.claude/evolution/CLAUDE.md`.

### US-005: Add propose skill skeleton
**Description:** As a developer, I want a dedicated propose skill so that each iteration starts with one focused hypothesis.

**Acceptance Criteria:**
- [ ] Create `.claude/skills/evolution-propose/` with the files needed for a Claude Code skill.
- [ ] The skill contract says it reads prior session artifacts and writes `hypothesis.md`.
- [ ] The skill contract says it updates `iteration.json` with hypothesis metadata.
- [ ] The skill contract says it returns a stop signal if no valid hypothesis exists.
- [ ] Skill files exist at the documented path.

### US-006: Add implement skill skeleton
**Description:** As a developer, I want a dedicated implement skill so that candidate changes are applied consistently from the selected hypothesis.

**Acceptance Criteria:**
- [ ] Create `.claude/skills/evolution-implement/` with the files needed for a Claude Code skill.
- [ ] The skill contract says it reads `hypothesis.md` and writes `implementation.md`.
- [ ] The skill contract says it records changed files in the implementation artifact.
- [ ] The skill contract says it updates `iteration.json` with implementation status.
- [ ] Skill files exist at the documented path.

### US-007: Add benchmark skill skeleton
**Description:** As a developer, I want a dedicated benchmark skill so that every candidate is validated consistently against the current baseline.

**Acceptance Criteria:**
- [ ] Create `.claude/skills/evolution-benchmark/` with the files needed for a Claude Code skill.
- [ ] The skill contract says it reads current iteration state and writes `benchmark.md`.
- [ ] The skill contract says it updates `iteration.json` with benchmark status and summary metrics.
- [ ] The skill contract explicitly requires at least one SPRT run with a minimum of 10 completed games.
- [ ] Skill files exist at the documented path.

### US-008: Add decision skill skeleton
**Description:** As a developer, I want a dedicated decision skill so that iteration outcomes are deterministic and auditable.

**Acceptance Criteria:**
- [ ] Create `.claude/skills/evolution-decide/` with the files needed for a Claude Code skill.
- [ ] The skill contract says it reads `implementation.md` and `benchmark.md` and writes `decision.md`.
- [ ] The skill contract says it updates `iteration.json` with one final state.
- [ ] The allowed final states are documented as: accepted, rejected, inconclusive, failed.
- [ ] Skill files exist at the documented path.

### US-009: Add isolated candidate workspace setup
**Description:** As a developer, I want each candidate implemented in isolation so that rejected changes cannot leak into the accepted baseline.

**Acceptance Criteria:**
- [ ] The orchestration flow creates an isolated candidate workspace, worktree, branch, or equivalent reversible environment for each iteration.
- [ ] The isolation path or identifier is stored in `iteration.json`.
- [ ] If setup fails, the iteration state becomes `failed` and the baseline remains unchanged.
- [ ] The next iteration starts from the latest accepted baseline, not the last attempted candidate.
- [ ] `bash -n scripts/evolution-loop.sh` passes.

### US-010: Add correctness gate step
**Description:** As a developer, I want obviously broken candidates rejected before benchmark time is spent on them.

**Acceptance Criteria:**
- [ ] The orchestration flow runs configured correctness checks before benchmarking.
- [ ] `iteration.json` records which checks ran and whether they passed.
- [ ] If correctness checks fail, no benchmark runs for that iteration.
- [ ] Failed correctness checks make the iteration ineligible for promotion.
- [ ] The correctness-gate behavior is documented in the project.

### US-011: Add benchmark policy contract
**Description:** As a developer, I want a documented validation policy so that promotion does not rely on ambiguous benchmark evidence.

**Acceptance Criteria:**
- [ ] The project documents screening versus confirmation benchmark policy.
- [ ] Screening requires at least one SPRT run with a minimum of 10 completed games.
- [ ] The policy defines when stronger validation is required before promotion.
- [ ] The policy defines what benchmark summary fields must be written into `iteration.json`.
- [ ] The benchmark policy is referenced by the benchmark and decision skills.

### US-012: Add iteration state machine
**Description:** As a developer, I want a single iteration state machine so that script and skills agree on valid transitions.

**Acceptance Criteria:**
- [x] The project documents valid iteration states and transitions.
- [x] `iteration.json` uses the documented state names.
- [x] The state machine distinguishes in-progress phase from final outcome.
- [x] Invalid transitions are treated as orchestration errors.
- [x] The state-machine rules are referenced by the orchestration script and decision skill.

### Iteration state machine contract

The orchestration script is the authority for iteration state transitions. Skills may update iteration metadata for their phase, but they must not invent new state names or skip required transitions.

#### In-progress states

- `initialized` — iteration artifacts exist and isolation metadata has been written.
- `proposing` — the propose phase is actively selecting a hypothesis.
- `proposed` — `hypothesis.md` and hypothesis metadata have been written.
- `implementing` — candidate code changes are being applied in the isolated workspace.
- `implemented` — implementation finished and `implementation.md` is ready.
- `validating` — orchestration-owned correctness checks are running.
- `benchmarked` — benchmark artifacts and summary fields are complete and ready for the decision phase.
- `deciding` — final outcome selection is in progress.

#### Final outcome states

- `accepted` — promotion evidence passed policy and the candidate becomes the new accepted baseline.
- `rejected` — evidence is strong enough to discard the candidate while keeping the baseline unchanged.
- `inconclusive` — the candidate finished evaluation but evidence is not strong enough for accept/reject.
- `failed` — orchestration, isolation, correctness, implementation, or benchmark infrastructure failed.

#### Valid transitions

```text
initialized -> proposing -> proposed -> implementing -> implemented -> validating
validating -> implemented        # correctness passed; benchmark remains eligible
validating -> failed             # correctness failed or orchestration error
implemented -> benchmarked       # benchmark artifact complete and ready for decision
benchmarked -> deciding -> accepted
benchmarked -> deciding -> rejected
benchmarked -> deciding -> inconclusive
```

A transition outside this graph is an orchestration error. The script must stop the iteration immediately rather than silently rewriting state.

#### `iteration.json` requirements

- `state` stores the authoritative current state using only the names listed above.
- `stateMachine.reference` points to this contract so downstream tooling can resolve the canonical rules.
- `stateMachine.currentPhase` mirrors `state` for explicit human-readable inspection.
- `stateMachine.finalStates` lists `accepted`, `rejected`, `inconclusive`, and `failed`.

#### Ownership rules

- The orchestration script enforces transition validity before it writes orchestration-owned states such as `validating` and `failed`.
- The decision phase must only write a final state after the iteration reaches `deciding`.
- Any state mismatch between the expected prior state and the observed `iteration.json` value is treated as a hard orchestration error.

The orchestration script references this contract via `STATE_MACHINE_REFERENCE`, and the decision skill must use the same state names and final-state rules.

---

### US-013: Add versioning policy contract
**Description:** As a developer, I want the version bump behavior defined clearly so that accepted candidates promote consistently.

**Acceptance Criteria:**
- [ ] The project documents where the engine version is stored.
- [ ] The project documents how an accepted iteration increments the version.
- [ ] The promotion artifact contents are documented.
- [ ] Rejected, failed, and inconclusive iterations are documented as non-version-bumping outcomes.
- [ ] The versioning policy is referenced by the decision flow.

### US-014: Add discard and restore contract
**Description:** As a developer, I want a documented discard path so that non-winning candidates can be removed safely.

**Acceptance Criteria:**
- [ ] The project documents how rejected, failed, and inconclusive candidates are discarded or isolated away.
- [ ] The discard path preserves iteration artifacts for audit.
- [ ] The discard path guarantees the accepted baseline remains unchanged.
- [ ] The next iteration start point after discard is documented.
- [ ] The discard policy is referenced by the orchestration script.

### US-015: Add loop control and stop conditions
**Description:** As a developer, I want the script to control iteration flow so that a full session can run unattended until a clear stop condition is reached.

**Acceptance Criteria:**
- [ ] The orchestration script continues automatically after each completed iteration unless a stop condition is reached.
- [ ] The script stops when `max-iterations` is reached.
- [ ] The script stops when no valid next hypothesis can be generated.
- [ ] The script stops when configured infrastructure failure limits are exceeded.
- [ ] `bash -n scripts/evolution-loop.sh` passes.

### US-016: Add final session summary artifact
**Description:** As a developer, I want a final summary file so that I can review the outcome of the session quickly.

**Acceptance Criteria:**
- [ ] The session writes a final summary artifact at the end of the run.
- [ ] The summary lists accepted versions, rejected attempts, and the stop reason.
- [ ] The summary includes pointers to per-iteration artifacts.
- [ ] The summary file path is documented in the project.
- [ ] The orchestration script writes the summary on both normal stop and early stop.

## 5. Functional Requirements

- FR-1: The project must provide a runnable orchestration script at `scripts/evolution-loop.sh`.
- FR-2: The project must provide worker guidance at `.claude/evolution/CLAUDE.md`.
- FR-3: The project must provide dedicated skills at the documented `.claude/skills/evolution-*` paths.
- FR-4: The orchestration script must create deterministic session and iteration artifact directories under `tasks/evolution-runs/`.
- FR-5: The orchestration script must be the only component that controls loop progression, stop conditions, promotion, and discard behavior.
- FR-6: Skills must communicate through artifacts and `iteration.json`, not hidden in-memory assumptions.
- FR-7: Each iteration must operate from the latest accepted baseline only.
- FR-8: Candidate implementation must happen in an isolated, reversible environment.
- FR-9: Every candidate must pass configured correctness checks before benchmark execution.
- FR-10: Benchmark validation must include at least one SPRT run with a minimum of 10 games.
- FR-11: The system must support stronger validation when the first benchmark result is too weak for promotion.
- FR-12: Allowed final outcomes are `accepted`, `rejected`, `inconclusive`, and `failed`.
- FR-13: Accepted candidates must automatically bump the engine version and become the new baseline.
- FR-14: Non-winning candidates must not modify the official baseline version.
- FR-15: Every iteration must persist hypothesis, implementation, benchmark, and decision artifacts.
- FR-16: Every session must persist a final summary artifact.
- FR-17: The harness must be documented well enough that a user can run the loop end to end from the repository.

## 6. Non-Goals (Out of Scope)


- Building a generic autonomous coding platform unrelated to engine evolution.
- Guaranteeing Elo gain on every single session.
- Automatically pushing branches, creating pull requests, or publishing releases.
- Building distributed benchmarking infrastructure.
- Changing unrelated repository areas unless required directly by the loop harness.

## 7. Technical Considerations

- Candidate ideas may include search changes, evaluation heuristics, move ordering, pruning, time management, transposition-table behavior, and simple parameter tuning.
- The benchmark policy should treat a 10-game SPRT as a screening step, not as universally sufficient promotion evidence.
- Recommended default policy:
  - screening SPRT with minimum 10 games;
  - confirmation benchmark when the first result is too weak or too early;
  - promotion only after the configured acceptance policy is satisfied.
- The worker prompt should explicitly limit scope so one iteration does not drift into unrelated refactors.
- The harness should avoid retrying recently rejected ideas unless configured to revisit them.
- Artifact formats should be simple enough to inspect manually and stable enough for automation.

## 8. Success Metrics

- A user can run one command and start an unattended evolution session.
- The repository contains the script, skills, and worker guidance needed to operate the loop.
- Every iteration produces an auditable record of what was attempted and why it was accepted or rejected.
- Accepted candidates always correspond to benchmark-backed promotions.
- Rejected candidates never leak into the official baseline.
- The loop stops predictably at `max-iterations` or another explicit stop reason.

## 9. Open Questions

- What should the default SPRT parameters be for screening and confirmation?
- Should the benchmark skill reuse the same opening set between screening and confirmation?
- What exact version string format should promoted engine versions follow?
- Should the loop keep a machine-readable registry of previously rejected ideas to prevent near-duplicate retries?
