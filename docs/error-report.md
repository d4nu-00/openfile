# Error report: blunder statistics and estimated rating

Status: spec written first on branch `error-report` (2026-10-16), stacked on
`tournament-report`; implemented as described below. The direction was
agreed in conversation: reuse the engine infrastructure of Tactical
Analysis, and port what can be traced.

Evidence is from CBase18.exe 18.1 (x64), analysed statically through an REA
session (`analysis/reports/` in the research workspace). Messages are cited
by their ordinal in the message catalogue.

## What ChessBase does

| Command | Id | Class, run slot | Key | Message |
|---|---|---|---|---|
| Error Report | `BLUNDER_REPORT` (kc 654) | `DBToolsBlunderReportCmd`, `0x1408b6a10` | B | "Statistical analysis of blunder types" (51560/51561) |

* **Starting it.** The command calls the active list's method at offset
  0x4c0, which reports on the list's selected games. The entry
  (`0x1407a82b0`) asks "Quick (normal games)" (51577) or "Slower (top
  games)" (51578) under the title "Error Report", and sets a global used as
  the analysis depth: 9 for Quick and 12 for Slower. Called with a flag
  instead of asking, it uses 10. The remarks say the analysis "spends only
  milliseconds on each move. This is here %2.1f ply" (110668).
* **Analysis** (`0x1403fe350`, per game `0x1404035c0`). Each move gets a
  loss and a weight:
  * The loss is the mover's evaluation before the move less the one after
    it, in centipawns, clamped to 0–400. It is 0 when the move played is the
    engine's best.
  * The weight depends on what kind of move it was, tagged in the code:
    `LEGAL` (the only legal move) 0, `RECAPT` (a recapture) 0.2, `EASY` 0.7,
    `MISS` 3, `BESTDEEP` 3, `DEEPSTRONG` 5, `MISSDEEP` 2.5, `MISSDEEPWIN` 5,
    `RELBLUND` 4. Otherwise it is 1 plus the position's complexity / 100
    (`CPLX`, at least 1 + 1). A move that misses the reaction to the
    opponent's blunder gets 2 more (`=>BLUNDERREACT`). When the loss is under
    10 and the move took over 250 (units not traced), the weight grows in
    proportion.
  * Games are cut off after 120 plies ("Cut Off at move %d"); "Dead Draw"
    stops the analysis of a game.
  * Per side it yields the centipawn loss (sum / moves) and the "Weighted
    Error Value" (4693, sum of loss × weight / sum of weights, once the
    weights pass 4).
* **Statistics** (`0x14046c460`, "Fill Statistics" 110635): more than 3,000
  games bring a warning. Errors are tallied by category (`0x14047c8a0`,
  "Blunder Categories" 4768), by context (`0x14046fc70`: "Under Attack", "My
  Attack", "Equal/Stronger/Weaker Opponent", "Opening", "Middlegame",
  "Endgame"), and by evaluation transition (`0x14046f580`, 15 kinds from
  "Stays winning" to "Loses quicker", 110675–110689). Each category keeps a
  count, a magnitude sum and a weight sum. It also keeps the average rating
  of the analysed side, and a per-game average that defaults to 79 when
  absent.
* **Blunder Elo** (110660, `0x1404705d0`). With *e* the error value
  (`0x14046f140`: the magnitude sum over the weight sum, plus up to 2 for a
  steep magnitude-vs-complexity slope above 0.27 when the fit's R² is over
  0.7, plus up to 4 for a distribution "shape" above 4):
  * base = 1.16 *e*² − 73 *e* + 3141.5 for *e* < 15; 3110 − 53.5 *e* for 15
    ≤ *e* ≤ 20; 2680 − 31 *e* above 20;
  * plus 2 × (*a* − 90), at least −20, where *a* is that per-game average;
  * plus, for *e* < 15 and a slope under 0.27, (1 − (*e* − 3) / 12) × (0.27 −
    slope) × 300;
  * rounded to tens, at least 500.

  The report shows "Blunder Elo = n (±d)", where d is the difference from the
  side's average rating (`0x14046fe70`).
* **Blitz probability** (110665, `0x14046ec10`): 1 / (1 + e^−(x − 4)/2), in
  percent, shown when the real rating exceeds the Blunder Elo by more than
  300 (`0x140478600`). *x* adds capped terms for shape, slope, error value
  over 14, "stays winning" and hanging pieces (`0x14046e740`).
* **The page** (`0x14047fe70`) is HTML with charts: magnitude vs
  complexity with its slope (110661, 110673), the magnitude distribution
  with its shape (110662, 110674), impact on result (110663), contexts
  (110664), categories, and observations (`0x14047c420`, 110644–110694). The
  charts compare with reference "GMs" distributions built into the program,
  and the remarks cite a statistical fit predicting 70 % of real ratings
  (110667).

**Not traced:** the complexity measure and the deep-search tags (they come
from ChessBase's own engine), the category and context thresholds, the
shape and slope definitions, the per-game average behind *a* (taken here to
be the analysed plies), and the observation rules.

## Workbench

### What is ported

* **Error Report** (B, database window) analyses the games selected in the
  game list, at most 1,000, with an engine of the user's (installed or a
  file), as Tactical Analysis does. The dialog offers ChessBase's **Quick**
  (depth 9) and **Slower** (depth 12), and whose moves to count: one player
  of the selection (by default the one in most games) or both sides.
* **The engine pass** runs in the backend, cancellable with progress, with
  one engine process. For each game it analyses the main line's positions
  up to ply 120 to the chosen depth (stopping at 2 s or 6 s per position
  at most). From those it gets each counted move's loss: the evaluation
  before less the evaluation after, from the mover's side, clamped to
  0–400, and 0 when the move is the engine's best. Unreadable games and
  set-ups the engine refuses are skipped and counted.
* **Weights**: ChessBase's 0 for the only legal move and 0.2 for a
  recapture (a capture on the square of the previous move's capture);
  every other move weighs 1. ChessBase's other tags need its engine's
  complexity and deep searches, which a UCI engine does not report.
* **Error value** *e* = Σ loss × weight / Σ weight over the counted moves.
  Because the other weights are 1, this is close to an average centipawn
  loss.
* **Blunder Elo**: ChessBase's curve of *e*, with *a* = the analysed plies
  per game. The slope and shape terms are left out (no complexity
  measure). It is shown with the side's average rating in the analysed
  games and the difference. The result is labelled an estimate, with
  ChessBase's caveat that it varies a lot with few games.

### The workbench's own statistics

These stand in for ChessBase's charts, whose thresholds were not traced:
* **Severity**, per 100 counted moves: inaccuracies (25–49 cp), mistakes
  (50–99), blunders (100–299) and losing blunders (300+). These are common
  conventions, not ChessBase's categories.
* **Magnitude distribution**: losses of 25 cp or more in bins of 25 up to
  400.
* **Contexts**: average loss and error count by phase (opening = the first
  15 moves, endgame = each side has at most 13 points of pieces,
  middlegame otherwise) and by opponent (stronger or weaker by 100 points or
  more, equal otherwise).
* **Evaluation transitions**, ChessBase's 15 kinds, for moves losing 50 cp
  or more, with the bands winning ≥ +2.00, better ≥ +0.70, equal, worse
  ≤ −0.70 and lost ≤ −2.00 (the workbench's bands).
* **Worst moves**: the 30 largest losses with the game, move and best move.
  Double-clicking one loads the game at that move.

### Not ported

The HTML page and its charts, the GM reference distributions (ChessBase's
data), the observation sentences, the blitz probability, the complexity
slope, the shape, and ChessBase's deep-search weights.

### Architecture

* `src-tauri/src/error_report.rs`:
  * pure functions from per-move records to the report (error value,
    Blunder Elo, severity, distribution, contexts, transitions), with unit
    tests;
  * the engine pass, which reuses `EngineSession`, the tablebase
    configuration and the option checks of Tactical Analysis. It stops each
    search when the depth is reached.
* Commands `error_report` (with a progress channel) and
  `cancel_error_report`, keyed by database, like Tactical Analysis's jobs.
* The selection's rows map to entry numbers with `ordinals_of_rows`. Games
  are read through the database's own read handle.
* Frontend: an Error Report dialog (engine, mode, side, progress and Stop,
  then the report). Mocks are in the harness and the demo API.
* Tests: pure statistics; an app test with `mock-uci legal` over a scratch
  2CBH; frontend tests for the dialog.
