# Opening classification and the opening report

Status: implemented on branch `eco-classification` (2026-10-15). The
direction was agreed in conversation: classify from open data only, never
from ChessBase's files.

Evidence is from CBase18.exe 18.1 (x64), analysed statically through an REA
session (`analysis/deep-analysis/` in the research workspace).

## What ChessBase does

| Command | Id | Run slot | Key | Message |
|---|---|---|---|---|
| Classify Opening | `CLASSIFYGAME` (`ClassifyGameCmd`) | `0x1409c5110` | none | "Show opening index of database for current game" |
| Opening classification (board) | `OPENINGCLASSIFYBOARDCMD` | `0x1408e1990` | Ctrl+Alt+C | none |
| Set ECO Codes | `DBTOOLSSETECOCODE` (`DBToolsSetEcoCodeCmd`) | `0x140a1fe40` | none | "Write ECO codes into games" |

* **The classifier** is a global object (`DAT_14228b748`), loaded from
  ChessBase's own ECO file: the strings `Eco.cbcod` and `Eco.cod` sit at
  `.rdata` file offsets 0x1274208 and 0x1274218. Its lookup (`0x140b54800`)
  copies the game, plays its main line up to a ply limit (object offset 0x6c,
  value not traced), then takes moves back until the position is in its
  table. It returns the packed ECO word (code and sub-classification) and the
  ply. That is the deepest known position of the main line.
* **Classify Opening** (`0x1409dcee0`) runs the classifier on the game and
  shows a dialog titled "ECO" (template 1245). The dialog has the board at
  that position, the opening's name (`0x140b6d270`) and the code as text
  (`0x140c48cf0`).
* **Set ECO Codes** (`0x1402ca4b0`) needs the database's classification
  object and then starts a job through the database window. The job itself
  was not followed.
* **The packed word** is at byte 128 of a 2CBH record (LE16) and in the
  classic record (BE16). Bits 7–15 hold 1–500 for A00–E99. The low seven
  bits are ChessBase's sub-classification, whose meaning comes from its ECO
  file (see [formats](formats.md)).

**Not traced.** The format of `Eco.cod`/`Eco.cbcod`, the ply limit, how the
sub-classification is chosen, and the opening index ("keys", `.cko`) that
the command's description mentions.

## Data and provenance

The workbench classifies with the
[lichess-org/chess-openings](https://github.com/lichess-org/chess-openings)
table, dedicated to the public domain under CC0 1.0:
* revision `a6189a30dc273ccb21fc2536a9a2fefd5592a67a` (2026-10-06);
* five TSV files (`a.tsv` to `e.tsv`, 396 KB, 3,865 lines of ECO code, name
  and moves in SAN);
* kept unmodified in `crates/chess-core/data/eco/` and compiled into the
  program. The licence text is in `licenses/chess-openings-CC0.txt` and the
  notice in `THIRD_PARTY_NOTICES.md`.

No ChessBase file is read: no `Eco.cod`, no key file and no book. The names
are lichess's, e.g. "Petrov's Defense" for C42. Every line of the table
replays legally; at first use each line's final position is indexed by a
Zobrist hash of the board, side to move, castling rights and a legal en
passant square.

## Workbench

* **Classification** (`chess_core::eco`) reads the main line from the
  standard start position, up to 60 plies or the first null move. It takes
  the last position the table holds, as ChessBase takes the deepest known
  position, so transpositions count. Set-ups and Chess960 games are not
  classified.
* **Classify Opening** (Ctrl+Alt+C, game window, Report tab) shows the code,
  the name, the move after which the position is reached and the game's own
  ECO tag. **Set ECO** stores the code in the game data as one edit.
* **Set ECO Codes** (database window, Maintenance › Improve) works on a 2CBH
  database opened for editing, of at most 20,000,000 entries. It is
  cancellable, with progress:
  1. Every game's main line is classified in parallel (deleted games too;
     text entries are skipped). Only the changes are kept, 10 bytes each.
  2. The codes that change are written to record byte 128, in chunks of
     50,000 with one sync each (`Writer::set_eco_many`). The opening tree's
     journal records the new file stamps.
  * **Only games without an ECO code** is on by default. The table agrees
    with ChessBase's own codes for about three games in four (see below),
    so existing codes are kept unless the user unticks it.
  * A game whose main code is already the table's keeps its whole word,
    ChessBase's sub-classification included. A new code is written with a
    sub-classification of 0.
  * Set-ups, games without moves and unreadable games keep their word.

  The report counts games, codes written, codes already right, other codes
  kept, games not classified, and how many of the coded games already had
  the table's code.

## Measurements

Release build on the development Mac, through
`cbh-reader/examples/eco.rs`, warm cache, read-only.

| Database | Games | Time | Same main code as ChessBase | Differ: same letter | Not classified |
|---|---|---|---|---|---|
| Mega 2025, 2CBH family | 24,759 | 0.26 s | 17,036 of 22,308 coded games (76.4 %) | 86.3 % | 2,451: 597 set-ups, 1,854 games without moves |
| Mega 2025, classic | 11,149,379 | 188.5 s (8 workers) | 7,545,408 of 11,003,269 (68.6 %) | 75.9 % | 146,095; 15 unreadable (the 12 castling set-ups that PR #22 makes readable, and the three damaged records) |

Every standard game with at least one move in the 2CBH sample was
classified. The differences come from the two tables: lichess's
assignments are not ChessBase's, and ChessBase's sub-classified table is
finer than five ECO volumes.

# Opening Report

Status: implemented on branch `opening-report` (2026-10-15).

## What ChessBase does

**Opening Report** (`OPENINGREPORTBOARDCMD`, `OpeningReportBoardCmd`, run slot
`0x1408e19d0`; "Create a detailed for the current board position", 50540;
no default key) calls `0x1407a9e60`. That picks the reference database
(`0x1407b19c0`) and creates the report (`OpeningReport`, vtable
`0x1411c5aa8`) as an HTML page in a report window
(`HtmlOpeningReportLink`). The generator is `0x1407075a0`. In order:

1. **Title.** "Opening report: " (50270), with the position's ECO code and
   opening name from the classifier above. A start position gives "Not
   enough moves" (10). Ten weights 7, 4, 2.5, 1, 0.9, 0.8, 0.7, 0.64, 0.55
   and 0.5 are set up (offset 0x770), and the current year is clamped to
   1998–2020 (offset 0x780). Neither use was followed.
2. **The games.** A position search of the database (`0x14070ac70`) fills
   a sorted list of game numbers, then a summary of the count
   (`0x140705cd0`).
3. **History** (50274, `0x140704f20`).
   * Fewer than 10 games are listed one by one.
   * Otherwise it shows the earliest game (50275), the latest annotated
     game (50278), the latest grandmaster game (50277) if it is another,
     and the latest game (50276) if it is yet another.
   * Then a 580×280 chart, "Games played in years" (50093).
   * Over 50 games it adds a second chart, the "Fashion index - Distribution
     of games relative to database" (50279). Its years per bar follow the
     count: 10 under 50 games, 5 under 301, 3 under 801, 2 under 2,001, and 1
     above.
   * The charts need the database's year statistics. Without them the
     report asks for the database to be installed on the hard disk (50554).
4. **Players** (81, `0x140706c50`), when the players have more than 6
   games.
   * a) "Strong grandmasters who used this line as" (50281) White or Black:
     up to 10, chosen by `0x140704620` (not followed).
   * b) "Other notable players" (50282): up to 8 players with more than a
     hundredth of the games or more than 10, or an average rating over 2500,
     and at least 2 games, not already listed.
   * Each row (`0x140709a90`) has the player, the games ("n="), the score,
     and the rating.
5. **Statistics** (448, `0x140707230`).
6. **Moves and plans**, after "Calculate plans?" (50273):
   * a book is built from the games (50345, `0x14083ef00`);
   * "Process moves" (50272) prepares 20 moves (`0x1407042a0`);
   * "Games + Moves" (50553) follows;
   * "Moves and Plans" (50283, `0x140706040`): each move with its games,
     rating and score, "played by" (50284) its strongest players (up to 5),
     then its plans (50280), "You should play" (50285), "Alternative"
     (50287) and "Click for games" (50286, `0x14070adf0`).
   * "Main line" (50289, `0x140708a30`) follows a move only while at least
     3 games play it; "Critical line" (50288) also exists.
7. **Opening keys** (909, `0x1407059b0`).

**Not traced.** The statistics block, how plans and "You should play" are
computed, the strong-grandmaster filter, the weights, and the HTML layout
beyond its headings.

## Workbench

**Opening Report** (game window, Report tab) reports on the board position
of the shown game, from the open database. It runs in the backend with
progress and Stop.

* **The games** come from the reference search: the opening tree, or Find
  Position (run even on a large database without a search booster, since
  the user asked). At most 100,000 games in database order are read; the
  report says when more reach the position.
* **Title:** the ECO code and name from the open table, and the line.
* **Results:** White wins, draws and Black wins.
* **History**, as ChessBase lays it out:
  * under 10 games, every game;
  * otherwise the earliest game, the latest annotated game, the latest
    strong game when it is another, and the latest game when it is yet
    another;
  * games per year;
  * over 50 games, the fashion index in ChessBase's bucket sizes: the
    bucket's share of the position's games over its share of the
    database's games (100 = average). Buckets with fewer than 100 database
    games get no index, and the chart caps bars at 400. Both are the
    workbench's choices, so a single old game does not dominate the
    chart.
  * "Annotated" means the game has an annotator, and "strong" that both
    players are rated 2500 or more. Both are the workbench's choices, since
    ChessBase's flags were not traced.
  * The database's games per year are counted once per entry count (a
    parallel scan of the entry records) and kept for the session.
* **Players:**
  * strong players by colour: rated 2500 or more, best first, up to 10 (the
    workbench's filter);
  * ChessBase's "Other notable players" rule, by games.
* **Moves.** Up to 20 move rows: from the opening tree when it covers the
  position, all games included, otherwise from the games read. "Played by"
  lists the five strongest players of each move.
* **Games read.** The moves after the position are read from the 5,000
  strongest games by rating.
* **Main line:** the most played move while at least 3 of the games read
  play it, up to 20 plies.
* **Not ported:** plans, "You should play", alternatives, the critical line,
  the statistics block, opening keys, photos and the HTML page. The report
  is a dialog.

### Measurements

Release build, through `src-tauri/examples/opening_report.rs`, read-only,
with the app's own opening-tree cache.

| Database | Position | Games | Time | Notes |
|---|---|---|---|---|
| 2CBH Mega | 1.e4 c5 | 1,523 | 4.5 s | no tree: Find Position over 24,783 entries; moves from the games read |
| Classic Mega | Najdorf (5...a6) | 100,000 of more | 50.8 s, then 23.8 s warm | the first run counts the years of 11.15 M entries; moves from the tree; ten strong players by colour |

Most of the warm time goes to reading 100,000 entry summaries and 5,000
games. The history of a truncated report covers the first 100,000 games in
database order: the Najdorf's then ends in 2009.
