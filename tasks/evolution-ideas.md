# Evolution Ideas

Use this checklist as input for `scripts/evolution-loop.sh --ideas-file`.
Only unchecked items (`- [ ]`) are considered pending ideas.

- [ ] Improve first movements using a book with opening from pro matches
- [ ] Implement transposition tables
- [ ] Improve move ordering by prioritizing captures with MVV-LVA before quieter moves.
- [ ] Improve move ordering by scoring killer moves higher in non-capture positions.
- [ ] Improve move ordering by adding a simple history heuristic for quiet moves.
- [ ] Improve search stability by refining aspiration window widening after fail-high or fail-low results.
- [ ] Reduce search overhead by avoiding repeated evaluation of clearly losing quiet moves late in move lists.
- [ ] Improve quiescence search by restricting low-value noisy moves that rarely change evaluation.
- [ ] Improve pruning in quiet positions by tightening futility pruning thresholds near leaf nodes.
- [ ] Improve pruning by making late move reductions slightly more aggressive for low-priority quiet moves.
- [ ] Improve pruning by reducing less aggressively when the side to move is in a tactically sharp position.
- [ ] Reduce allocation or temporary object churn in the search loop hot path.
- [ ] Reduce repeated board-derived computations by caching frequently reused search state within a node.
- [ ] Improve transposition table usefulness by adjusting replacement preference toward deeper or newer entries.
- [ ] Improve transposition table move ordering by preferring stored best moves earlier in node expansion.
- [ ] Improve evaluation speed by short-circuiting obviously drawn or materially trivial positions earlier.
- [ ] Improve time efficiency by skipping expensive follow-up work when the node budget is nearly exhausted.
