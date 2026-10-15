# Opening classification

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
