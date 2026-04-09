## Overview

Implemented passed-pawn evaluation in the Wiggum Engine.

## Implementation Details

- **Evaluation**: Added passed-pawn detection for both colors within `evaluate(board)`, on top of the existing material evaluation.
- **Passed pawns**: A pawn is considered passed when there are no enemy pawns ahead of it on the same file or on either adjacent file.
- **Bonus scaling**: Passed pawns now receive a progressive bonus based on advancement, so more advanced passed pawns are valued more highly.
- **Perspective**: The passed-pawn bonus preserves the current evaluation convention — positive when the side to move is better, negative when worse.
- **Scope**: The change is local to `chess-engine/src/eval.rs` and does not depend on alpha-beta pruning or piece-square tables.

## Known Limitations

- Passed-pawn evaluation is intentionally simple and does not yet account for king support, blockades, rook-behind-pawn heuristics, or promotion-race calculation.
- Bonus values are static and rank-based only; they are not adjusted by game phase or tactical context.
- No interaction yet with broader positional terms such as isolated pawns, doubled pawns, connected passed pawns, or piece-square tables.