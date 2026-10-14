# Player statistics

Status: implemented on branch `player-statistics` (2026-10-14).

## What ChessBase does

Evidence is from CBase18.exe 18.1 (x64), its command table, messages and
dialog templates.

| Command | Id | Default key | Message |
|---|---|---|---|
| Statistics | `STATISTICS` (kc 589) | S | "Statistics" (50132/50133) |
| Player statistic | `PLAYERSTATISTIC` (kc 634) | none | "Shows statistics about a player (opponents, tournaments, openings)" (50918/50919) |

* **Player statistic** is `PlayerStatisticCmd` (run slot `0x1408b7dc0`). It
  opens `CPlayerStatisticDialog` (vtable `0x1411df548`, constructor
  `0x1407c66b0`) on dialog template 846, "Player statistic". The template
  has three list boxes side by side, each with buttons below it:
  * left (1093): **N**, **A-Z**, **Score** (1091, 1048, 1049);
  * middle (1094): **Score**, **A-Z**, **Date**, **Table** (1050–1052, 1056);
  * right (1095): **White**, **Black**, **A-Z** (1053–1055);
  * an **Elo** button (1090) and **OK**.

  The tooltip names the three lists: opponents, tournaments and openings.
  The dialog's set-up (`0x1407c7650`) builds a heading from "Statistics for"
  (50134), "Games" (3232) and "from" (900).
* The opening list is filled by `0x1407c6aa0`. Each item has a key word
  (formatted by `0x140c48890`), then the games as White and their score, and
  the games as Black and theirs. The score is half points × 50 / games,
  followed by "%". The list has tab stops at 20 and 60.
* **Statistics** (S) is `StatisticsCmd` (run slot `0x1408b87e0`). It hands
  the active list window's selection to `0x1408c0670`. The graphical
  statistics dialog `CStatistik2DDialogEx` (vtable `0x1411c5480`) names
  Length, AvLen, Years, Eco-A…Eco-E, Endgames and Survival views.

**Not traced:**
* the opponent and tournament item formats and the Elo button's display;
* whether the opening key is an ECO code or a ChessBase opening key (the
  formatter `0x140c48890` was not followed);
* the drawing of `CStatistik2DDialogEx`.

The textual player report messages (50091–50118, e.g. "scores", "performs",
"against an opposition of") belong to the dossier, which is not ported.

## Workbench

**Opening it.**
* **Statistics** (S, database window) shows the player chosen in the
  Players list.
* **Player statistic** (database and game ribbons) shows the chosen player,
  or else the White player of the shown game. When it starts from a game,
  buttons switch between the game's two players.

**Finding the games.** One parallel scan of the entry index (up to 8
threads, each with its own read handle) collects the player's games, as
either colour. It reads each record's ids, date, result, ratings and ECO,
and decodes no game. It reports progress, can be cancelled from the dialog,
and keeps at most 100,000 games. Deleted games are counted and left out.

The scan was preferred to the sort orders after measuring both (see
below). A sort order by White and one by Black do keep a player's games
together, but each game in the run then costs a record read and a name
lookup.

**Scoring.**
* Wins, draws and losses count for the player; forfeits score as their
  result.
* Lines, "0-0" and unknown result codes are counted as "without a result"
  and not scored.
* **Score** is points per game in percent.
* **Elo-Av** is the average rating of the rated opponents, forfeits left
  out.
* **Perf.** is that average plus FIDE's rating difference for the score
  against them. It uses the same table as the crosstable.

**Layout**, following dialog 846:
* A table of the score as **White**, as **Black** and in **Total**: N, +, =,
  −, points, score, Elo-Av and performance.
* A **Years** table: the player's highest rating that year, games, score
  and performance. This stands in for ChessBase's Elo button, whose display
  was not traced.
* Three lists side by side, each with ChessBase's buttons:
  * **opponents**: rating in the latest rated game between them, games,
    score; sorted by N, A-Z or Score;
  * **tournaments**: year, games, score, performance; sorted by Score, A-Z
    or Date (the default, latest first). **Table** shows the chosen
    tournament's crosstable;
  * **openings** by ECO: games and score as White, then as Black; sorted by
    White, Black or A-Z (code order). ECO is used because opening keys are
    not read; codes outside A00–E99 count as none.
* The opponent and tournament lists keep at most 5,000 rows each; the
  dialog says how many were left out.

**Listing games.** Double-clicking any row lists its games in the game
list, e.g. "Carlsen, Magnus: against Nakamura, Hikaru". The rows are colour,
total, a year, an opponent, a tournament or an opening. The backend keeps
the player's games from the scan, so no second scan is needed. They are
dropped when the database is closed or changed.

## Measurements

Release build on the development Mac (10 cores), classic Mega 2025
(11,151,119 entries), warm cache, `cbh-reader/examples/player_stats.rs`:

| Player | Games | Scan (8 threads) | Scoring and names |
|---|---|---|---|
| Carlsen, Magnus | 6,925 | 0.056–0.066 s | 0.04–0.06 s |
| Kasparov, Garry | 2,735 | 0.063 s | 0.038 s |
| Nakamura, Hikaru | 9,288 | 0.063 s | 0.066 s |

Finding Carlsen's games through sort orders by White and by Black took
0.14 s, so the scan was kept. The first run, which also built the player
list, took 2.5 s in all; its peak memory was 278 MB, almost all of it from
building the list.
