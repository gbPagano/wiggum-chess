# Wiggum Engine v0.2 — Changes

## Search
- Replaced plain Negamax/Minimax traversal with Alpha-Beta pruning.
- Added alpha-beta window propagation through the recursive search.
- Added root-level alpha-beta handling while preserving the public `search(board, depth)` API.
- Added pruning cutoffs when `alpha >= beta`.

## Correctness / Terminal handling
- Preserved terminal evaluation for checkmate and stalemate.
- Added a regression test to ensure terminal positions return immediately without expanding children.

## Impact
- Same search result semantics as the previous minimax/negamax implementation.
- Lower search tree expansion in many positions, enabling faster analysis at the same depth.

## Notes
- Engine version bumped to `0.2.0`, which exposes itself as `Wiggum Engine v0.2` in UCI.