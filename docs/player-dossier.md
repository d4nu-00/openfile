# Player dossier

Status: implemented on branch `player-dossier` (2026-10-15), stacked on
`prepare-against`. The direction was agreed in conversation: port what the
user's databases can supply, without photos or online data.

Evidence is from CBase18.exe 18.1 (x64), analysed statically through an REA
session (`analysis/reports/` in the research workspace). Messages are cited
by their ordinal in the message catalogue.

## What ChessBase does

| Command | Id | Class, run slot | Key | Message |
|---|---|---|---|---|
| Dossier | `PLAYERDOSSIER` (kc 588) | `PlayerDossierCmd`, `0x1408b7c80` | Ctrl+Alt+D | "Create a dossier about a player" (50129/50130) |
| Dossier (encyclopedia) | `PLBASE_DOSSIER` (kc 804) | `PlBaseDossierCmd`, `0x140911310` | none | "Create a dossier for the selected player" (51091) |
| (AI assistant) | — | `GeneratePlayerDossierCmd`, `0x140899270` | — | "Generation of dossier about <name> was successful." |

* **Starting it.** The command calls the active list's method at vtable
  offset 0x400. In the player list that is `0x14073b640`, which hands the
  chosen player's name to `0x1407ab010`. That looks the player up in the
  player encyclopedia (`0x1406bf400`). Depending on the answer it starts the
  generator in the background (`0x1406548f0` with `0x1407a9f00`) or opens
  the older dossier (`0x1407b2220`; "Legacy Dossier", 51576). The dossier is
  an HTML page ("Dossier about", 50206, `0x140829050`).
* **The generator** is `0x1408358a0`. It shows a progress dialog titled
  "Dossier" (50087) and writes these sections in order, each announced in
  the progress dialog:
  1. **Title**: "Dossier" and the player's first and last name
     (`0x140834f00`).
  2. **Collect games** (4108, `0x140837320`). It finds the player by name
     (or a second spelling) and takes their game list (`0x140132cb0`). Games
     before a year limit (object offset 0x7b4) are dropped. The rest are
     split into games as White and as Black (`0x140b18d80`), deleted games
     left out.
  3. **Biography** (50088, `0x140832180`), from the player encyclopedia;
     "Player encyclopedia is not available" (50090) otherwise.
  4. **Pictures** (50089, `0x140833370`).
  5. **Elo Profile** (50058, `0x140833710`): a rating graph ("EloGraph")
     from the encyclopedia's rating history, with "Individual development"
     (50094) and the player's age ("at age", 50095).
  6. With at least one game, **Statistics** (50131): games per year
     ("Games played in years", 50093, `0x140834320`) and the tallies of
     `0x1408369a0`. See below.
  7. **Tournaments** (473, `0x140834fb0`), when an option (0x7b8) is set.
  8. **Opponents** (1509, `0x140832e30`).
  9. With the repertoire option (0x7a8): "Build tree from white games"
     (50792, `0x140831ce0`) and from Black's (50793, `0x140831ad0`), then
     "Repertoire for white" (50790, `0x140835780`) and "for black" (50791,
     `0x1408329f0`). These use two board windows.
  10. **Spectacular games** (50092, `0x140834010`): a search over the
      games as White, then as Black. The search sets a 4-byte value {3, 0,
      0, 0} at search offset 0xec0 (`0x140b3d390`) and 4 at offset 0x60.
      Which criteria these are was not traced.
  11. **Games with mate** (50794, `0x140832b10`), when an option (0x7bb) is
      set: another search over each colour's games. It sets flags at search
      offsets 0x109d and 0x10d0; they were not decoded.
  12. **Annotated games** (50795, `0x140831f40`): the player's games whose
      annotator is the player ("annotates his games", 50091).
* **Statistics** (`0x1408369a0`, `0x14082ef70`, `0x140838a30`). Only games
  with a normal result (1-0, draw, 0-1) count:
  * For all games, those as White and those as Black, each game adds to the
    White wins, draws or Black wins and to their total moves. A draw under
    20 moves is a quick draw. Ratings are summed by colour.
  * The player's score in a list is (2 × wins + draws) × 50 / games, rounded
    (`0x14083a080`).
  * By the opponent's rating, in bands of 50 from 1000 to 2999: games,
    points and the opponents' rating sum.
  * The opponents, tournaments and ECO codes are tallied with the player's
    points.
* **The verdicts**, each band table read from `.rdata`, a value falling in
  `low ≤ v < high`:
  * Score (`0x140839cf0`, table `0x1411f7880`): the score less 55 with
    White and 45 with Black, in the bands −100, −20, −8, −2, 2, 8, 20, 100.
    The seven words run from 50096 to 50102 and follow "scores" (50103),
    with the score in brackets.
  * Performance (`0x140839660`): "performs" (50104), the performance,
    "against an opposition of" (50105), the opponents' average and the
    difference.
  * Length (`0x140838f90`, table `0x1411f78b0`): the average moves of White
    wins, draws and Black wins (50113, 50112, 50111) in the bands 0, 25,
    32, 38, 43, 48, 55, 999, with seven words (50106–50110, 391, 392).
  * Drawing quote (`0x140838af0`, table `0x1411f78d0`): draws in percent of
    the games in the bands 0, 20, 28, 34, 38, 47, 100, with six words
    (50114–50116, 1112, 80, 1111), then "quick draws" (50118) in percent.

**Not traced:** the spectacular-games and mate search criteria, the year
limit and option defaults, the encyclopedia lookup's answers, the HTML
layout, and the legacy dossier.

## Workbench

**Dossier** (Ctrl+Alt+D; database window Home › Lists, game window Home ›
Database) builds the dossier of the player chosen in the Players list, or
else the shown game's White player, from the open database. It runs in the
backend with progress (games, repertoire, mates) and Stop.

The dossier dialog shows, in ChessBase's order:
* **Title:** the player, games, years, and deleted games left out.
* **Statistics:** White, Black and all games, each with games, White wins,
  draws, Black wins, the score, and the opponents' average and performance
  (ChessBase's score and FIDE's table).
  * Sentences give the score's band and the performance with each colour,
    the length band of White wins, draws and Black wins, and the drawing
    quote with quick draws, over all games.
  * The band boundaries are ChessBase's. The words are the workbench's own
    (e.g. "slightly above expectation"), since ChessBase's are its text.
* **Elo profile and games played in years:** the player's highest rating
  each year from the games, with games and score. This stands in for the
  encyclopedia's rating graph.
* **Score by the opponent's rating**, in ChessBase's bands of 50.
* **Tournaments** (latest first) and **opponents** (most games first), at
  most 30 each. Rows list their games.
* **Repertoire for White** and **for Black**: the lines of
  [Prepare](preparation.md), from the main lines of each colour's games.
* **Games with medals**: games with any medal but the user medal (bits
  0–14; see [formats](formats.md)), with their medals. This stands in for
  "Spectacular games", whose criteria were not traced.
* **Mates given** and **mates received**: decisive games whose main line
  ends in checkmate. The latest 20,000 decisive games are read.
* **Annotated by the player**: games whose annotator's name equals the
  player's, case and spaces ignored.
* The game lists show the latest 30 with their full count; double-clicking
  loads a game.

**Not ported:** the biography, pictures and rating history (the player
encyclopedia), the year limit and options, and the HTML page.

The player's games are kept as for Player statistic, so the dossier's rows
list their games without another scan.

## Measurements

Release build, `src-tauri/examples/player_dossier.rs`, read-only, with the
app's data folder for the player list.

| Database | Player | Games | Cold | Warm |
|---|---|---|---|---|
| Classic Mega 2025 | player A | 6,925 | 22.4 s | 0.62 s |
| 2CBH Mega 2025 | player C | 446 | — | 0.26 s |

Player A has 14 + 12 repertoire lines, 174 games with medals, 192 mates
given and 47 received among 4,697 decisive games, and 26 games they
annotated.
