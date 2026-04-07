---
name: evolution-decide
description: "Decide the final outcome of the current iteration. Use during the decision phase to evaluate implementation and benchmark results and determine if the candidate is accepted, rejected, inconclusive, or failed. Triggers on: decide, evaluate outcome, evolution decide."
---

# Evolution Decide Skill

Determine the final outcome of the current iteration and persist the decision.

---

## The Job

1. Read `implementation.md` and `benchmark.md` from the current iteration directory
2. Evaluate the candidate against the acceptance policy
3. Write `decision.md` with the outcome and reasoning
4. Update `iteration.json` with one of the allowed final states

---

## Inputs

- `iteration.json` in the current iteration directory (required) — contains iteration number, baseline version, hypothesis, implementation, and benchmark metadata
- `hypothesis.md` in the current iteration directory — the proposed improvement
- `implementation.md` in the current iteration directory — summary of candidate changes
- `benchmark.md` in the current iteration directory — benchmark results
- `correctness/results.md` in the current iteration directory — correctness-gate results and benchmark eligibility

---

## What to Do

1. **Read All Artifacts** — Open `implementation.md` and `benchmark.md` to review what was implemented and how it performed. Open `iteration.json` for the full context including hypothesis, state, and metrics.

2. **Evaluate Against Acceptance Policy** — Determine the outcome based on:
   - **Accepted**: The implementation succeeded, the benchmark evidence satisfies the documented policy, `benchmark.sufficientForPromotion` is `true` with a statistically meaningful improvement over the baseline, and the candidate can be promoted under the versioning policy.
   - **Rejected**: The implementation succeeded and benchmark evidence is strong enough to show the candidate is weaker than or equivalent to the baseline. The candidate is discarded.
   - **Inconclusive**: The implementation succeeded but the benchmark evidence is insufficient for a clear accept/reject decision (for example: only screening evidence, weak signal, high variance, or confirmation still required). The candidate may be refined in a future iteration.
   - **Failed**: The implementation, correctness gate, or benchmark infrastructure failed (build error, test failure, benchmark crash). No evaluation of the candidate's merit is possible.

3. **Write `decision.md`** — Record the decision with:
   - The final outcome (accepted, rejected, inconclusive, failed)
   - The reasoning for the decision
   - Key evidence cited from the benchmark results, including `policyStage`, completed games, and whether `benchmark.sufficientForPromotion` was satisfied
   - For an `accepted` outcome, the promotion metadata required by the versioning policy (`previousVersion`, `promotedVersion`, baseline refs, and version-artifact path)
   - Any recommendations for future iterations

4. **Update `iteration.json`** — Follow the canonical state-machine contract in `tasks/prd-wiggum-evolution-loop.md`:
   - Enter the decision phase only from `benchmarked` by transitioning to `deciding` before choosing an outcome
   - Set the final `state` to exactly one of `"accepted"`, `"rejected"`, `"inconclusive"`, or `"failed"`
   - Keep `stateMachine.currentPhase` aligned with the final `state`
   - Add `decision` object with:
     - `outcome` — one of the allowed final states
     - `reasoning` — explanation of the decision
     - `evidence` — key metrics that supported the decision, including benchmark policy fields used for the call

---

## Decision Format

The decision artifact should follow this structure in `decision.md`:

```markdown
# Iteration N Decision

## Outcome

One of: accepted, rejected, inconclusive, failed.

## Reasoning

Explanation of why this outcome was selected based on the evidence.

## Evidence

- Implementation: summary of what was changed
- Benchmark: key results (games completed, win rate, ELO estimate)
- Policy: which acceptance criteria were met or not met

## Recommendations

Suggestions for future iterations (e.g., "retry with more games", "explore related idea from iteration X", "abandon this direction").
```

---

## Allowed Final States

| State | Meaning | Baseline |
| --- | --- | --- |
| `accepted` | Candidate validated and promoted | Updated to new version |
| `rejected` | Candidate evaluated and discarded | Unchanged |
| `inconclusive` | Evidence insufficient for clear decision | Unchanged |
| `failed` | Implementation or benchmark infrastructure failure | Unchanged |

The final-state rules come from the iteration state machine in `tasks/prd-wiggum-evolution-loop.md`. The decision phase must not skip directly from `implemented` to a final outcome.

---

## Scope Constraints

- Only write `decision.md` and update `iteration.json`; do not modify engine code, benchmark results, or other iteration artifacts
- Do not persist or discard candidate changes in the codebase — that is handled by the orchestration script
- Do not bump version numbers — that is handled by the orchestration script on accepted candidates

---

## Output Contract

After this skill runs, the following must be true:

- `decision.md` exists in the iteration directory with the outcome and reasoning
- `iteration.json` has been updated with one of the four allowed final states: `accepted`, `rejected`, `inconclusive`, or `failed`
- `iteration.json` includes a `decision` object with `outcome` and `reasoning` fields

---

## References

- Worker guidance: `.claude/evolution/CLAUDE.md`
- Evolution loop: `scripts/evolution-loop.sh`
- Benchmarks: `scripts/benchmark-version.sh`
- Benchmark policy: see US-011 benchmark policy contract
- Versioning policy: see US-013 versioning policy contract in `tasks/prd-wiggum-evolution-loop.md`
- Iteration state machine: `tasks/prd-wiggum-evolution-loop.md`
