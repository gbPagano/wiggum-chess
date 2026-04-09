# Wiggum Engine v0.3 — Changes

## Evaluation
- Added Piece-Square Tables (PST) for pawns, knights, bishops, rooks, queens, and king.
- Kept the existing material evaluation and extended it with positional bonuses based on piece placement.
- Applied PST scoring with correct board-orientation handling for White and Black pieces.
- Preserved the current `evaluate(board)` contract: positive scores favor the side to move.

## Correctness / Terminal handling
- Preserved special-case evaluation for checkmate and stalemate.
- Kept terminal positions returning fixed scores before any material or positional evaluation is applied.
- Added evaluation tests covering positional effects introduced by PSTs.

## Impact
- Improves positional understanding beyond raw material count.
- Encourages more natural development and piece activity, such as knight centralization and better king placement.
- Should improve move quality especially at shallow search depths, where static evaluation has higher influence.

## Notes
- No public API changes were made.
- The change is scoped to static evaluation and does not require search architecture changes.
- Engine version can be bumped to `0.3.0` if this release is intended to expose PST-based evaluation as a new engine revision.