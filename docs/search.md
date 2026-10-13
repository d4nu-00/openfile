# Searching databases

This note fixes how the workbench searches whole databases. ChessBase's search
mask (`OPEN_SEARCHMASK`) has tabs for game data, position, material and more;
the field names below follow its game-data tab (dialog resource 1031 in
CBase18.exe 18.1). Nothing here reads or writes ChessBase's own search files
(Search Booster `.cit`/`.cib`, `.cbtt`); sources stay read-only.

## Game data

A filter combines any of these fields; every given field must match:

| Field | Matches |
|---|---|
| White, Black | Last name and first name, each a case-insensitive prefix of the stored name |
| Ignore colours | The two player fields may match either colour |
| Wins only | The named player won (with Ignore colours: either side) |
| Tournament | Case-insensitive substring of the event or the site |
| Annotator | Case-insensitive substring of the annotator's name |
| Elo | Range for none, one, both or the average of the two ratings |
| Year | Inclusive range; games without a year never match a year range |
| ECO | Inclusive range of codes, A00–E99 |
| Moves | Inclusive range of the stored main-line move count |
| Result | Any of 1-0, 0-1, ½-½, 0-0; forfeits count as their PGN result |

Text entries never match. Deleted games match unless the filter excludes them.

**How it runs.** Name fields are resolved once: the reader scans the name
table (CBP/CBT/CBC or 2LID) and collects the matching ids. The entry index
(CBH or 2CBH) is then read in large chunks, and each record's ids, date,
ratings, ECO, result and move count are compared. No game is decoded. The
scan can be cancelled, reports progress, and stops after a configurable
number of hits (2,000,000 by default; the result says when it stopped).

The result is a list of entry numbers kept by the app. The game list then
pages through it in place of the whole database.

## Position search

Find Position (game window, Find Position › Hard Disk) looks for the board
and side to move of the current position. Castling rights, en passant and
move numbers are ignored, as in ChessBase. It looks through every game,
including variations. Each hit records the first node showing the
position, so the game opens there.

**Search booster.** Create Search Booster (Maintenance) builds the
workbench's own index. It is written to the app's data folder
(`search-boosters/`), never beside the database. The index holds one
128-byte Bloom signature per entry, made of the game's *structure keys*: a
hash of the pawn placement, the piece counts of both sides and the side to
move. These keys change only when a pawn moves or something is captured, so
a game has few of them. A search reads the signatures, keeps the games whose
signature can hold the target's key, and replays only those.

* An index records the size and modification time of the entry index and the
  move file. It is not used once either changes, for example after saving a
  game.
* A cancelled build leaves nothing behind.

**Replay.** A candidate is replayed depth first. A line is abandoned once
either side has fewer pawns, or fewer men, than the target, since neither
can come back (promotion only turns a pawn into a piece). Without a booster
every game is replayed. Position searches keep at most 100,000 hits.
