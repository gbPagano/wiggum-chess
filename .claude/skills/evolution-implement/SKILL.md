---
name: evolution-implement
description: "Implement a candidate engine change from the current iteration hypothesis. Use during the implementation phase to apply code changes and record what was modified. Triggers on: implement, apply changes, write implementation, evolution implement."
---

# Evolution Implement Skill

Apply candidate changes to the Wiggum engine based on the selected hypothesis and record what was done.

---

## The Job

1. Read `hypothesis.md` from the current iteration directory
2. Apply the candidate changes to the engine codebase within the isolated workspace
3. Write `implementation.md` summarizing the changes and listing every modified file
4. Update `iteration.json` with implementation status and changed files

---

## Inputs

- `hypothesis.md` in the current iteration directory (required) — the proposed improvement to implement
- `iteration.json` in the current iteration directory (required) — contains iteration number, baseline version, and artifact paths
- The engine codebase in the isolated workspace at the current baseline version

---

## What to Do

1. **Read Hypothesis** — Open `hypothesis.md` to understand the single focused change being proposed, why it should help, and what metrics will validate success.

2. **Apply Changes** — Implement the candidate changes in the engine codebase:
   - Limit scope to the change described in `hypothesis.md` — do not drift into unrelated refactors
   - Follow existing code patterns and conventions
   - Do not modify files unrelated to the engine's performance or correctness
   - If the hypothesis cannot be implemented (e.g., prerequisite changes are missing), record the failure instead of partially implementing

3. **Write `implementation.md`** — After applying changes (or determining implementation is not feasible), write the implementation file with:
   - A summary of what was implemented
   - A list of every file that was added, modified, or deleted
   - Any deviations from the hypothesis and why
   - If implementation failed, the reason and what blocked it

4. **Update `iteration.json`** — Patch the state and add implementation metadata:
   - On success: set state to `"implemented"`, add `implementation` object with:
     - `summary` — brief description of what was done
     - `changedFiles` — array of paths for each modified/added/deleted file
   - On failure: set state to `"failed"`, add `implementation` object with:
     - `summary` — what was attempted
     - `failureReason` — why it could not be completed
     - `changedFiles` — empty array or any partial changes made

---

## Implementation Format

The implementation artifact should follow this structure in `implementation.md`:

```markdown
# Iteration N Implementation

## Summary

Brief description of what was implemented.

## Changed Files

- `path/to/modified/file.rs` — short description of the change
- `path/to/new/file.rs` — brief note

## Deviations

Note any changes that diverge from the hypothesis and why, OR "No deviations — implementation matches the hypothesis exactly."

## Notes

Optional additional context for reviewers or the benchmark skill.
```

---

## Scope Constraints

- Only modify engine code, tests, benchmarks, or build scripts directly related to the hypothesis
- Do not change unrelated modules, CI configurations, or user-facing documentation
- Do not bump version numbers — the orchestration script handles versioning on accepted candidates
- Do not create or modify git branches — the isolated workspace is managed by the orchestration script

---

## Output Contract

After this skill runs, the following must be true:

- `implementation.md` exists in the iteration directory with change details OR a failure reason
- `iteration.json` has been updated with:
  - state set to `"implemented"` (success) or `"failed"` (blocked)
  - `implementation` object with `summary`, `changedFiles`, and optionally `failureReason`

---

## References

- Worker guidance: `.claude/evolution/CLAUDE.md`
- Evolution loop: `scripts/evolution-loop.sh`
- Benchmarks: `scripts/benchmark-version.sh`
- Benchmark policy: see US-011 benchmark policy contract