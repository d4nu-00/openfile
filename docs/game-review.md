# Guided game review

The Centipawn Analysis dialog now presents a review after its engine pass. It shows estimated accuracy for each side, the existing move category totals, a compact timeline of every analysed move, and a list of inaccuracies and worse moves. Each critical move includes the engine's suggested continuation in SAN when the engine supplied a legal line. Clicking a move closes the dialog and selects that position on the board. Reopening the command shows the saved review without another engine pass while the game's main line is unchanged.

The review uses the same position evaluations and classifications as Centipawn Analysis. The backend retains each position's principal variation from the UCI engine and converts legal moves to SAN. It sends a review item with the game node, ply, SAN, centipawn loss, category and suggested line for each analysed move. The existing annotated game texts are still applied as one undoable edit.

Estimated accuracy is the arithmetic mean of `100 × exp(−loss / 150)` over the analysed moves for a side, where `loss` is the existing, capped centipawn loss (0 to 400). It is a simple, transparent scale for this review, not a calibrated win probability. Forced moves and opening moves are handled as in Centipawn Analysis: the opening is skipped, and analysed forced moves are included.

The game review intentionally follows the existing game window and analysis dialog styling. Its move cards are short and scroll horizontally; the full critical findings sit below. The move labels and engine continuations use algebraic notation rather than UCI coordinates.
