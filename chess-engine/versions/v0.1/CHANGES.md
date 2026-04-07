# Wiggum Engine v0.1 — Changes

## Overview

Initial implementation of the Wiggum Engine.

## Implementation Details

- **Evaluation**: Material evaluation only — piece values (pawn=1, knight=3, bishop=3, rook=5, queen=9) summed for each side; no positional evaluation.
- **Search**: Negamax search to a fixed depth of 4 with no alpha-beta pruning.
- **Move ordering**: None — moves are generated in pseudo-random bitboard order.

## Known Limitations

- No alpha-beta pruning (significant performance headroom available).
- No quiescence search (evaluation can be unstable at leaf nodes with captures available).
- No move ordering (alpha-beta would benefit greatly from this).
- Fixed depth only — no time-based search control.
