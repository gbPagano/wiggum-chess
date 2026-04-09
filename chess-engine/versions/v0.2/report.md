# v0.2 — Version Report

_Generated: 2026-04-09_

## Highlights

- Replaced plain negamax/minimax traversal with alpha-beta pruning.
- Preserved terminal handling for checkmate and stalemate.
- Kept the public `search(board, depth)` API while reducing tree expansion in many positions.

## Match Results

| Opponent / Scenario | Games | Wins | Draws | Losses | Win% |
|---------------------|-------|------|-------|--------|------|
| Wiggum Engine v0.1 (startpos, STC) | 10 | 5 | 5 | 0 | 50.0% |
| Wiggum Engine v0.1 (balanced positions, STC) | 10 | 4 | 5 | 1 | 40.0% |
| Stockfish 17 (1500-STC, startpos) | 10 | 3 | 0 | 7 | 30.0% |
| Stockfish 17 (1500-STC-balanced) | 10 | 3 | 2 | 5 | 30.0% |

**Overall vs v0.1:** 20 games, 9 wins, 10 draws, 1 loss — **45.0% win rate**

**Overall vs Stockfish 1500:** 20 games, 6 wins, 2 draws, 12 losses — **30.0% win rate**

## SPRT / Summary Artifacts

- `stc.csv`: `Wiggum Engine v0.2` vs `Wiggum Engine v0.1` — 10 games, 5 wins, 5 draws, 0 losses, `sprt_result = inconclusive`.
- `stockfish.csv`: despite the filename, the artifact also contains `Wiggum Engine v0.2` vs `Wiggum Engine v0.1` — 10 games, 2 wins, 8 draws, 0 losses, `sprt_result = inconclusive`.

## Notes

- Available artifacts show that v0.2 is stronger than v0.1 in direct STC matches.
- Alpha-beta pruning improves search efficiency, but the available SPRT summaries are still inconclusive.
- No higher-level Stockfish report artifact was available beyond the two 1500-STC result rows above.
