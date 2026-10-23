# Centipawn Analysis

Status: implemented on branch `centipawn-analysis` (2026-10-18); the database
list version on branch `centipawn-list` (2026-10-22). The first was stacked on
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
| Achievement Analysis | `ACHIEVEMENT_ANALYSIS` (kc 184) | none registered | | "Perform Achievement Analysis on a game" (4819/4820) |

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
* **The database list version** (`0x1403fef00`, the list's slot 0x5e8),
  traced on 2026-10-22:
  * It asks for its options in Tactical Analysis's settings dialog
    (`0x14042b3f0`, titled 4682), and above 100,000 games asks "Are You
    Sure?" (3027) with "Replacing games in a big database can take time"
    (4205).
  * It skips deleted games and games of one ply or less. Each other game is
    analysed (`0x1404161f0`). With Tactical Analysis's Replace setting
    (`V_REPLACE`) the annotated game replaces its entry; without it, the game
    is appended.
  * The per-game analysis (`0x1404035c0`) returns a value for White and Black
    and a game weight:
    * the value ("BV") is the side's Weighted Error Value when both sides'
      weights pass 4, otherwise its centipawn loss, rounded;
    * the weight is the sum of the 40 largest per-move complexities (the
      `CPLX` behind the Error Report's move weights).
  * The per-game analysis also stores the game's evaluations as an
    evaluation profile (type 0x26, `0x140c4e370` → `0x1401a86d0`; see
    [evaluation profile](evaluation-profile.md)).
  * **Statistics** (`0x14041e730`): a map keyed by player name, with commas
    made spaces. Each game adds a record for both players: opponent, opponent
    rating, result in half points, value and weight. A player keeps the rating
    of their first game.
  * **The files** (`0x14041faf0`), rewritten after each game:
    "<database>-CentiPawnLoss.csv" and a ".txt" with one summary line per
    player (`0x14041b280`: the values joined by "/", then "=> Average=…").
    The CSV has the columns:
    * Player Name, Elo, Perform, Points;
    * BV-1…BV-n, one value per game, blanks up to the player with the most
      games;
    * Weighted Av: Σ value × weight / Σ weight (`0x140419610`), printed
      × 0.01 with two decimals;
    * Opponent-1…Opponent-n: "name rating points wgt=weight" (`0x14041b480`).

    A player's games are in descending weight (`0x1403f2ac0`), and the
    players in ascending Weighted Av (`0x1403ee9a0`).
  * **Perform** (`0x1404191c0`, `0x140c492c0`): over games against rated
    opponents, their average rating plus 400 log10(p / (1 − p)) for the score
    p (a whole percentage), −800 at 0 % and +800 at 100 %.
  * **Points** (`0x140419290`) is printed "%1.1f"; the decompiler did not
    recover its floating-point result, so the sum of the results is assumed.

**Achievement Analysis** was searched for again on 2026-10-22, without
result. Its menu text (4819) is used nowhere and its tooltip (4820) only by
the command table (`0x14094d530`). No command object is registered for kc 184:
the registrations store each command's kc as an immediate (`0x1409b9640` and
others), and no store of 184 exists, while kc 183 (Centipawn Analysis) has
three. The command looks unused in this build, so there is nothing to port.

**Not traced:**
* the winning and losing thresholds (`0x140b57140`, `0x140b56f90`);
* the blunder flag, the sacrifice value and the deep-search flags, which
  come from the reference app's own engine;
* the margin of category 4;
* the complexity values that make the list version's game weights.

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
* **Not ported here:** the evaluation profile is not stored by the game
  window version; the list version stores it.

### Database list

* **Centipawn Analysis** in the database window (Home › Analysis jobs) runs
  on the selected games of a 2CBH database opened for editing. It asks for
  the engine and Seconds, as in the game window.
* **Each game** is checked as for analysis jobs. A game is skipped when it:
  * is open in a tab;
  * is marked as deleted;
  * has annotations that were not all decoded;
  * is a Chess960 game;
  * has no moves after its opening.

  Each other game is analysed with one engine process for the whole run, and
  gets the game window's texts. Its evaluations become its evaluation profile.
  The annotated game then replaces its entry, unless the entry changed during
  the analysis. Replacing is always on (the reference app's Replace setting),
  since appending would duplicate the games.
* **Bounds.** At most 100,000 games per run, where the reference app asks for
  confirmation. Stop ends the run after the current position; the games done
  so far stay replaced. An engine failure stops the run and keeps them too. At
  most 200 skipped games are listed, with their count.
* **Statistics** follow the reference app's map, values and columns. Two
  differences:
  * The game weight is the 40 largest of the workbench's move weights (the
    Error Report's port: 2, a recapture 0.2, an only move 0) × 100, since
    the reference app's complexity is not available from a UCI engine.
  * The Weighted Av is printed as the weighted mean itself, not × 0.01. With
    rounded error values as values, the reference app's form would give
    0.05 for an error value of 5; whether its values are scaled was not
    traced.

  Opponent names have commas made spaces, like the players', so that the
  CSV keeps its columns. A player's CSV row lists at most 1,000 games.
* **The dialog** shows the players (name, Elo, Perform, Points, games,
  Weighted Av), lowest Weighted Av first as in the CSV, with the skipped
  games.
  **Save CSV…** saves the CSV where the user chooses, by default
  "<database>-CentiPawnLoss.csv". The reference app's ".txt" summary is not
  written.

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

**Database list** (2026-10-22). A scratch 2CBH of 20 consecutive classic
test-database games (entries from 11,000,000, copied and deleted after),
Stockfish 17, Seconds 0 (200 ms per position), release build:
* 19 games were analysed and replaced in 274 s, about 14 s a game. One was
  skipped, having no moves after its opening.
* 37 players went into the CSV (38 lines with the header).
* The values ranged from 5 to 29. Most game weights were 8,000: the
  workbench's weight 2 for 40 moves.

Release build, `src-tauri/examples/centipawn_analysis.rs`, the classic test
database read-only, Stockfish 17 (`/opt/homebrew/bin/stockfish`),
Seconds 1 (750 ms per position):

| Game | Opening skipped | Positions | Time | Loss W/B | Error value W/B |
|---|---|---|---|---|---|
| entry 11,000,000 | 10 plies | 64 | 49.1 s | 17 / 44 | 9.09 / 23.56 |
| entry 11,100,000 | 10 plies | 51 | 39.3 s | 63 / 48 | 33.69 / 27.89 |
| entry 10,500,000 | 18 plies | 65 | 50.1 s | 20 / 8 | 11.34 / 4.15 |

That is about 0.77 s per position.
