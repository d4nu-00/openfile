# Searching databases

This note fixes how the workbench searches whole databases. ChessBase's search
mask (`OPEN_SEARCHMASK`, dialog resource 310) has tabs for game data,
position, material and more. The field names below follow its game-data tab,
dialog resource **311** in CBase18.exe 18.1. Earlier versions of this note
called it 1031, which is the resource's language (German), not its number.
Nothing here reads or writes ChessBase's own search files (Search Booster
`.cit`/`.cib`, `.cbtt`); sources stay read-only.

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
| Difference | At most this rating difference between the players; both must be rated |
| Year | Inclusive range; games without a year never match a year range |
| ECO | Inclusive range of codes, A00–E99 |
| Moves | Inclusive range of the stored main-line move count |
| Result | Any of 1-0, 0-1, ½-½, 0-0; forfeits count as their PGN result |
| Mate, Stalem., Check | The main line ends in one of those set; Check means check that is not mate |
| Full Text, NOT | Case-insensitive text in any text annotation, before or after a move, in any language; with NOT, games without it |

Text entries never match. Deleted games match unless the filter excludes them.

**Difference.** In dialog 311, "&Difference:" is an auto-checkbox (control
1079, style `0x50010003`) beside one number field (1023) with an up-down
(1024), in the column of the upper Elo bound. The workbench reads it as a
maximum rating difference. This reading follows the layout and was not
confirmed in ChessBase's code. The tab has no source field.

**Final position and text.** Mate, Stalem. and Check are controls 1074–1076;
Full Text and NOT are 1022 and 1081. A game is read only once it passes every
other field:
* for the final position, its move tree is decoded and the main line's end
  is tested;
* for text, only its annotation record is read.

Two things were not traced: whether ChessBase reads the main line's end or a
stored flag, and whether its Check includes mate. The workbench's choices are
in the table above.

**Not ported from dialog 311:**
* Text with Good and Beautiful (1073, 2338, 2339): their meaning was not
  traced.
* In Repertoire (1897): it needs a repertoire database.
* Time Controls All/Slow/Rapid/Blitz (1315–1319): the game type is not read
  from the records.

**How it runs.** Name fields are resolved once: the reader scans the name
table (CBP/CBT/CBC or 2LID) and collects the matching ids. The entry index
(CBH or 2CBH) is then read in large chunks by up to 8 workers, each with its
own read handle and range. Each record's ids, date, ratings, ECO, result and
move count are compared. Games themselves are read only for the final
position and text fields.
* The status bar shows the progress; its Cancel link, or Stop Process,
  stops the search.
* A search stops after a configurable number of hits, 2,000,000 by default,
  and the result says when it stopped. The hits kept are the first in entry
  order.

The result is a list of entry numbers kept by the app. The game list then
pages through it in place of the whole database.

## Position search

The search mask's Position tab is ChessBase's pattern search: three boards,
mirroring and a move window. See [position search](position-search.md). This
section covers Find Position, and the exact search that the tab shares with
it.

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
signature can hold one of the targets' keys, and replays only those.

* An index records the size and modification time of the entry index and the
  move file. It is not used once either changes, for example after saving a
  game.
* It is built by up to 8 workers, each writing its own range of signatures
  into the file in place.
* A cancelled build leaves nothing behind.

**Replay.** A candidate is replayed depth first. A line is abandoned once
either side has fewer pawns, or fewer men, than the target, since neither
can come back (promotion only turns a pawn into a piece). Without a booster
every game is replayed. Position searches keep at most 100,000 hits. The
work is split across up to 8 workers like the game-data search. A worker
stops once the ranges before it hold 100,000 hits, since its own hits would
be dropped.

## Measurements (classic Mega, 2026-10-14)

Release build on the 10-core development Mac, warm cache, 11,151,119
entries. Timed with `cbh-reader/examples/search.rs` and `position.rs`.

| Search | Hits | Time |
|---|---|---|
| White "Carlsen" | 3,837 | 1.1 s (3.5 s before the parallel scan) |
| White "Carlsen", Mate | 157 | 1.2 s |
| Year from 2024, Mate | 51,707 of 431,116 games | 32 s |
| Mate, whole database | 769,024 | 126 s |
| Full Text "novelty" | 3,726 | 1.7 s |
| Find Position, Opera Game after 4...Bxf3, no booster | 1,035 | 130 s |
| The same with the booster | 1,035 | 2.6 s (32,769 games replayed) |
| Ruy Lopez after 3.Bb5, no booster | 100,000 (limit) | 108 s (651 s before the read fix below) |
| The same with the booster | 100,000 (limit) | 16 s |
| Pattern: a white knight on d5, no black pawn on c6, c7, e6 or e7, moves 15–40, length 20 | 52,905 | 135 s |
| Building the search booster | 1.43 GB file | 128 s (about 35 min single-threaded, estimated) |

**Reads.** Profiling the first parallel version showed that most of the
time went to the kernel. Every read checked the source file twice by its
path (a `stat`, and an open for the file's identity), and moved a shared
cursor. Now:
* reads are positional, with no shared cursor;
* the path is checked at most every 200 ms;
* every operation still starts with a full check.

On the 2CBH Mega a full Find Position went from 2.73 s to 0.65 s on one
worker, and to 0.17 s on eight.
