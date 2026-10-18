# Centipawn Analysis

Status: implemented on branch `centipawn-analysis` (2026-10-18), stacked on
`generate-repertoire`. The direction was agreed in conversation: reuse the
Error Report's engine pass on a single game, and port what the reference app
shows or annotates.

Evidence is from the reference binary (x64), analysed statically through an
REA session. Messages are cited by their ordinal in the message catalogue.

## What the reference app does

| Command | Id | Class, run slot | Key | Message |
|---|---|---|---|---|
| Centipawn Analysis (game window) | `CENTIPAWN` (kc 183) | `CentipawnAnalysisCmd`, `0x1409fff70` | Ctrl+Alt+Shift+Z | "Analyses games for precision play in comparison to engine moves (and detect cheating)" (4682/4683) |
| Centipawn Analysis (database list) | | `DBCentipawnAnalysisCmd`, `0x1408b6830` | | 4682 |
| Achievement Analysis | `ACHIEVEMENT_ANALYSIS` (kc 184) | not found | | "Perform Achievement Analysis on a game" (4819/4820) |

* **Starting it** (`0x140a09480`). It asks for "Seconds" (1960), with a
  default from the settings. The time per position is 750 ms per second
  asked, at least 500 ms, or 200 ms for 0; a second value of 70 goes with it.
  It then builds the same analysis object as the Error Report
  (`0x1403f4fd0`) and runs it on the game (`0x1404161f0` → `0x1404035c0`).
  The window's title is "Centipawn Analysis - Variations" (4682, 497).
* **The opening is skipped.** Analysis starts after the ply where the ECO
  table's classification of the game ends (`0x140b54ed0`), and never before
  ply 10. Without a classification it starts at the first move.
* **The summary** (`0x1403f7d40`, at the game's start) gives two lines:
  * the Weighted Error Value (4693) for White and Black, with two decimals;
  * the centipawn loss (4689), *w* and *b*, rounded.

  The error value is the Error Report's (see [error report](error-report.md)).
  It is given once the weights pass 4.
* **Move categories** (`0x140408270`, `0x140418d40`). Each analysed move
  after the opening gets a category; its name goes after the move as text.
  For the three worst kinds, the loss in pawns follows ("%1.1f", or Mate).
  A table at the start then counts each category for White and Black, worst
  first. The categories (`0x140418b10`) and the rule:

  | # | Name (message) | Rule |
  |---|---|---|
  | 1 | Brilliant (110300) | the engine's move, with a sacrifice value from its deep search |
  | 2 | Strong (3168) | the engine's move, with a deep-search flag |
  | 3 | Best (3169) | the engine's move otherwise ("Easy" (339) added when easy) |
  | 4 | OK (20) | the engine's second line, within a margin of 7 (`0x14041c1f0`) |
  | 7 | Missed win (110304) | winning before, not after, and over 160 cp lost |
  | 8 | Loses game (110305) | losing after, not before, and over 160 cp lost |
  | 6 | Mistake (110303) | the analysis's blunder flag |
  | 5 | Inaccurate (110302) | over 25 cp lost and over 0.3 × the evaluation |

  The rules are tested in that order; a move fitting none gets no category.
* **The database list version** (`0x1403fef00`, the list's slot 0x5e8):
  * it asks for its options and warns above 100,000 games;
  * it analyses each selected game that is not deleted and replaces it with
    the annotated game;
  * it writes per-player statistics to "<database>-CentiPawnLoss.csv"
    (`0x14041e730`, `0x14041faf0`), with the columns Player Name, Elo,
    Perform, Points, BV-n, Weighted Av and Opponent-n.

**Not traced:**
* the winning and losing thresholds (`0x140b57140`, `0x140b56f90`);
* the blunder flag, the sacrifice value and the deep-search flags, which
  come from the reference app's own engine;
* the margin of category 4;
* Achievement Analysis. Its message appears only in the command table, and
  no handler was found.

## Workbench

* **Centipawn Analysis** (Ctrl+Alt+Shift+Z; game window, Analysis ›
  Tactical Analysis) runs on the shown game.
  * The engine is the user's, installed or a file.
  * The dialog asks for the reference app's Seconds, 0 to 60, and turns them
    into its time per position.
  * The job runs in the backend, cancellable with progress, on the main
    line up to ply 120, skipping the opening by the reference app's rule
    with the open ECO table (see [openings](openings.md)).
* **Each move** gets the Error Report's loss and weight, and a category by
  the reference app's rule as far as a UCI engine allows:
  * The engine's move is the **engine's move** (category 3). Brilliant and
    Strong need the reference app's deep-search flags, so they are never
    given.
  * **Good alternative** (4) is never given either: it needs a second line,
    and the pass asks for one.
  * **Lets the win slip** (7) and **Losing move** (8) use the reference app's
    160 cp. The workbench chooses ±2.00 as winning and losing (the Error
    Report's bands).
  * **Mistake** (6) stands for the untraced blunder flag. The workbench
    uses a loss of 100 cp or more.
  * **Inaccuracy** (5) is the reference app's rule.
* **The texts**, applied as one undoable edit:
  * The workbench's own sentence before the first move: average loss, error
    value, and the count of each category for both sides, worst first.
  * After each error (or each classified move, when asked), its category,
    with the loss in pawns for the three worst. Category names are the
    workbench's own words.
* The dialog then shows the same figures as tables.
* **Not ported:** the database list version and its CSV file, and
  Achievement Analysis.

### Architecture

* `src-tauri/src/centipawn.rs`:
  * the time rule, the classifier and the summary text, with unit tests;
  * the job, which reuses `error_report::evaluate_line` (the engine pass,
    shared with the Error Report) and `error_report::move_loss`.
* `Edit::AddAnalysis` now takes a text for the game's start (node 0); a
  symbol or line there is still refused.
* Frontend: `CentipawnDialog`, with mocks in the harness and the demo API.
* Tests: unit tests, an app test with `mock-uci` (every counted move loses
  34 cp, an inaccuracy), and a frontend test for the dialog and the edit.

## Measurements

Release build, `src-tauri/examples/centipawn_analysis.rs`, the classic test
database read-only, Stockfish 17 (`/opt/homebrew/bin/stockfish`),
Seconds 1 (750 ms per position):

| Game | Opening skipped | Positions | Time | Loss W/B | Error value W/B |
|---|---|---|---|---|---|
| entry 11,000,000 | 10 plies | 64 | 49.1 s | 17 / 44 | 9.09 / 23.56 |
| entry 11,100,000 | 10 plies | 51 | 39.3 s | 63 / 48 | 33.69 / 27.89 |
| entry 10,500,000 | 18 plies | 65 | 50.1 s | 20 / 8 | 11.34 / 4.15 |

That is about 0.77 s per position.
