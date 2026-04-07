---
name: evolution-propose
description: "Propose a single focused hypothesis for the current evolution iteration. Use at the start of each iteration to select what to improve. Triggers on: propose, suggest improvement, generate hypothesis, evolution propose."
---

# Evolution Propose Skill

Generate one focused hypothesis for the current Wiggum engine evolution iteration.

---

## The Job

1. Read prior session artifacts (previous iterations, session summary, accepted baselines)
2. Formulate a single hypothesis about what change could improve the engine
3. Write the hypothesis to `hypothesis.md` in the iteration directory
4. Update `iteration.json` with hypothesis metadata
5. Return a stop signal if no valid hypothesis can be formed

---

## Inputs

- Current `iteration.json` (required) — contains iteration number, baseline version, and artifact paths
- Prior session artifacts — previous `hypothesis.md`, `implementation.md`, `benchmark.md`, and `decision.md` files from earlier iterations
- Session root `summary.md` and `session.env` if available
- The current state of the engine codebase at the accepted baseline

---

## What to Do

1. **Read Prior Artifacts** — Review all completed iteration directories to understand what has already been tried, what was accepted, what was rejected and why.

2. **Identify an Opportunity** — Based on prior results, find one focused area for improvement. Good hypotheses target:
   - Move generation speed (table sizes, lookup patterns, algorithmic changes)
   - Evaluation accuracy (term weighting, new features, tuning)
   - Search efficiency (pruning thresholds, ordering heuristics, cutoffs)
   - Memory access patterns (cache-friendly layouts, prefetching)

3. **Write `hypothesis.md`** — The hypothesis file must include:
   - A clear one-sentence description of the proposed change
   - Why this change is expected to help (the mechanism of improvement)
   - What metrics should be measured to validate success
   - References to any prior iterations this builds on or learns from

4. **Update `iteration.json`** — Set the state to `"proposed"` and add a `hypothesis` object with:
   - `summary` — the one-sentence description
   - `reasoning` — why this should help
   - `targetMetrics` — list of metrics to track (e.g., "speed_perft_nodes_per_second", "bench_score")
   - `buildsOn` — iteration numbers this builds on, or empty array if independent

5. **Stop Signal** — If no valid hypothesis can be formed (prior iterations exhausted all viable ideas, or the engine is believed to be at a local maximum), write `hypothesis.md` explaining why, update `iteration.json` with `"no_hypothesis"` as the state, and inform the orchestrator.

---

## Hypothesis Format

The hypothesis should follow this structure in `hypothesis.md`:

```markdown
# Iteration N Hypothesis

## Proposed Change

One clear sentence describing what will change.

## Why This Helps

Brief explanation of the mechanism — why should this improve speed, accuracy, or search efficiency?

## Target Metrics

- Metric 1: expected direction (e.g., "nodes/sec should increase by >2%")
- Metric 2: expected direction

## Related Work

- References to prior iterations that informed this hypothesis OR "None — this is the first proposal"
```

---

## Output Contract

After this skill runs, the following must be true:

- `hypothesis.md` exists in the iteration directory with a valid hypothesis
- `iteration.json` has been updated with state `"proposed"` and hypothesis metadata
- OR `iteration.json` has state `"no_hypothesis"` with explanation

## References

- Worker guidance: `.claude/evolution/CLAUDE.md`
- Evolution loop: `scripts/evolution-loop.sh`
- Benchmarks: `scripts/benchmark-version.sh`
