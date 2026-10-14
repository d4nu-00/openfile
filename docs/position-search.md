# Position search in the search mask

Status: spec and implementation on branch `search-mask` (2026-10-14). The
direction (port ChessBase's position tab as it works, keep the exact Find
Position) was agreed in conversation.

## What ChessBase does

Evidence is from CBase18.exe 18.1 (x64). The search mask (dialog 310) has a
Position tab, dialog **314**, owned by `CPositionDialog` (vtable
`0x1411e4d00`, constructor `0x1407e6cb0`, data exchange `0x1407e93d0`,
OK `0x1407e98c0`). Its controls:

* a board, drawn by `0x1407e96c0`, and a palette of 7 × 2 piece buttons
  (1090–1097, 1105–1111);
* radio buttons **'Look for' board** (1083, message 309), **'Or' board**
  (1084, 311) and **'Exclude' board** (1085, 310; the template says "Not
  board"). They choose which of three boards the editor shows;
* **Mirror**: **Horizontal** (1086) and **Vertical** (1087);
* **First** (1098), **Length** (1113) and **Last** (1116), move numbers;
* **Sacrifice** (1302), **Copy board** (1088) and **Reset** (1089).

The outer mask (dialog 310) has **Include lines in search** (1285).

**Boards.** On OK, each board is turned into one 64-bit board per piece code
(`0x1401821b0`): codes 1–7 for White and 0x11–0x17 for Black. Codes 1–6 are
K, Q, N, B, R, P, as in the CBG set-up. Code 7 is matched against all of a
side's pieces, so it means "any piece". A square may hold several codes; the
board draws them as small pieces in a 2 × 2 grid. Bit *n* is square
`file × 8 + rank`, as in the CBG set-up.

**Matching** (`0x14018afb0`, for each piece code, White and Black alike, with
*P* the position's squares holding that piece, *L* the 'Look for' board,
*X* the 'Exclude' board and *O* the 'Or' board):

* `((¬X ∧ P) ∨ L) = P`: every square of *L* holds the piece, and no
  square of *X* does unless *L* also has it;
* if *O* is not empty, at least one square of *O* holds the piece.

Squares not on any board may hold anything, so the search finds a
**pattern**, not a whole position. Empty squares are asked for with "any
piece" on the 'Exclude' board.

**Mirroring** (compiled by `0x140188c90`, tried by `0x14018acd0`):

* Horizontal (flag 1): ranks 1↔8 and the colours swapped, i.e. the same
  pattern for the other side;
* Vertical (flag 2): files a↔h, colours kept;
* both: also the two together.

The game matches when the boards as given, or any mirrored set that is
switched on, match.

**Moves** (`0x14018ae80`). The positions from ply `2 × First − 2` up to,
but not including, ply `2 × Last` are tried. A run of at least **Length**
consecutive matching positions is needed. The game opens at the run's first
position. OK forces First ≥ 1, Length ≥ 1 and Last > First. A side-to-move
field (offset 0x68c of the criteria; 2 means either side) skips positions
with the other side to move; where the dialog sets it was not traced.

**Sacrifice** (offset 0x68d) also needs the next move to pass a test in
`0x14018b170` (`0x140c2d570`, built with 100, 100, 90). That test was not
traced. A quick pawn test (`0x14018b2c0`) compares the pawns before the full
test.

**Not traced:** the dialog's default values, how the boards map onto the
radio buttons (inferred from the matching rules, which are unambiguous), how
Include lines walks variations, and the Sacrifice test.

## Workbench

The Position tab gets a board editor in place of the FEN field.

* **Boards.** Three boards, 'Look for', 'Or' and 'Exclude', chosen as in
  ChessBase. Clicking a square with a palette piece adds it to the square, and
  clicking again removes it. A square holds up to four pieces, drawn in a
  2 × 2 grid. The palette has K, Q, R, B, N, P and **any piece** for each
  colour, plus an eraser. Copy board puts the current board on 'Look for';
  Reset clears the shown board.
* **Exact position** (a workbench option). It is on when the tab opens with the current board, and after Copy board.
  The 'Look for' board is searched as a whole position with its side to
  move, as Find Position does, through the search booster when there is one.
  In this mode the Or and Exclude boards are not used. Mirrors are allowed:
  each mirrored position is a further target, and the booster keeps the
  games whose signature can hold any of them.
* **Pattern** (exact position off): ChessBase's rules above, with Mirror,
  First, Length, Last and a side-to-move choice (either, White, Black).
* **Include lines in search** (off by default): variations are searched as
  well, each line from the start, its moves numbered as played. Off, only the
  main line is searched. Exact searches keep looking in variations, as Find
  Position always has.
* **Sacrifice** is shown disabled: its test was not traced.

**Booster.** It is used only for exact positions. A pattern leaves most
squares free, so its pawn structure and material are not known and the
booster's keys cannot be computed. Patterns therefore replay every game, but
only up to move Last, and every worker of the parallel scan replays its own
range.

**Bounds.** 100,000 hits, as for Find Position. Searches show progress and
can be cancelled from the status bar.

## Parallel scans

Game-data searches, Find Position, pattern searches and building the search
booster all split the database into one range per worker. There are up to 8
workers, each with its own read handle (and its own booster handle). The hits
of the ranges are joined in entry order, so a search that stops at its limit
keeps the first hits.

## Measurements

See [search](search.md#measurements-classic-mega-2026-10-14): on the
classic Mega a pattern search replays every game in about 135 s on 8
workers; an exact position with the booster takes seconds.
