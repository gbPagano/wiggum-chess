---
name: evolution-benchmark
description: "Run benchmark validation for the current iteration candidate. Use during the benchmark phase to validate changes against the baseline engine. Triggers on: benchmark, validate performance, run match, evolution benchmark."
---

# Evolution Benchmark Skill

Run benchmarks against the candidate engine changes and record the results.

---

## The Job

1. Read current iteration state from `iteration.json`
2. Run configured benchmark matches against the baseline engine
3. Write `benchmark.md` with benchmark settings, completed games, and summary metrics
4. Update `iteration.json` with benchmark status and summary metrics

---

## Inputs

- `iteration.json` in the current iteration directory (required) — contains iteration number, baseline version, implementation state, and artifact paths
- `hypothesis.md` in the current iteration directory — the proposed improvement being validated
- `implementation.md` in the current iteration directory — summary of candidate changes
- The engine codebase with candidate changes applied in the isolated workspace

---

## What to Do

1. **Read Current State** — Open `iteration.json` to confirm the iteration is in the `implemented` state and that `correctness.benchmarkEligible` is `true` before benchmarking. Open `hypothesis.md`, `implementation.md`, and `correctness/results.md` to understand what changed and whether the candidate passed the correctness gate.

2. **Build Engines** — Ensure both the candidate and baseline engines compile:
   - Build the candidate from the current isolated workspace
   - Identify the baseline engine binary or path from the session configuration
   - If builds fail, record the failure and mark benchmarking as failed

3. **Run Benchmark Matches** — Execute match series between the candidate and baseline using the documented benchmark policy:
   - Start with a **screening** SPRT (Sequential Probability Ratio Test) run with a **minimum of 10 completed games**
   - Use the chess-runner match command or the equivalent orchestration benchmark command
   - Configure time controls appropriate for the engine type
   - If the screening result is weak, early, or otherwise ambiguous, run a **confirmation** benchmark with stronger evidence (more games, longer time control, or both) before claiming the result is sufficient for promotion

4. **Write `benchmark.md`** — Record the benchmark outcome with:
   - Benchmark settings (time control, increment, opponent, game count)
   - Completed games count
   - Match result (wins, losses, draws for each engine)
   - SPRT result if available (ELO estimate, confidence interval, pass/fail)
   - Any anomalies or errors encountered during benchmarking

5. **Update `iteration.json`** — Patch the state and add benchmark metadata:
   - On success: set state to `"benchmarked"`, add `benchmark` object with:
     - `status` — `"completed"`
     - `policyStage` — `"screening"` or `"confirmation"`
     - `settings` — object with `timeControl`, `increment`, and `gamesRequested`
     - `metrics` — object with `gamesCompleted`, `candidateWins`, `baselineWins`, `draws`, `candidateWinRate`, `scorePerGame`, `eloEstimate`, and `sprtResult`
     - `sufficientForPromotion` — `true` only when the benchmark evidence satisfies the documented promotion policy
   - On partially completed or inconclusive runs: set state to `"benchmarked"`, include available metrics with notes and leave `sufficientForPromotion` as `false`
   - On complete failure (no games finished): set state to `"failed"`, add `benchmark` object with `failureReason`

---

## Benchmark Format

The benchmark artifact should follow this structure in `benchmark.md`:

```markdown
# Iteration N Benchmark

## Settings

- Time control: <time_ms> + <increment_ms>
- Games: <completed> / <requested>
- Candidate engine: <version/commit>
- Baseline engine: <version>

## Results

| Metric | Value |
| --- | --- |
| Candidate wins | |
| Baseline wins | |
| Draws | |
| Candidate win rate | |
| ELO estimate | |
| SPRT result | |

## Notes

Any additional observations.
```

---

## Minimum Requirements

- **At least 10 completed games** for the screening SPRT validation
- Screening evidence alone is not automatically sufficient for promotion
- If the screening result is weak, early, or otherwise ambiguous, run a confirmation benchmark and keep `benchmark.sufficientForPromotion` as `false` until the stronger evidence is complete
- If fewer than 10 games complete due to infrastructure issues, mark the iteration as a benchmark failure

---

## Scope Constraints

- Only run benchmark matches; do not modify engine code or iteration artifacts beyond `benchmark.md` and `iteration.json`
- Do not adjust time controls or game counts to game the result
- Do not create or modify git branches — the isolated workspace is managed by the orchestration script

---

## Output Contract

After this skill runs, the following must be true:

- `benchmark.md` exists in the iteration directory with benchmark results OR a failure reason
- `iteration.json` has been updated with:
  - state set to `"benchmarked"` (completed) or `"failed"` (infrastructure failure)
  - `benchmark` object with `status`, `policyStage`, `settings`, `metrics`, and `sufficientForPromotion`, or `failureReason` on total failure

---

## References

- Worker guidance: `.claude/evolution/CLAUDE.md`
- Evolution loop: `scripts/evolution-loop.sh`
- Benchmarks: `scripts/benchmark-version.sh`
- Benchmark policy: see US-011 benchmark policy contract
