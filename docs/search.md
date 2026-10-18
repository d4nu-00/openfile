# Searching databases

This note fixes how the workbench searches whole databases. The reference app's search
mask (`OPEN_SEARCHMASK`, dialog resource 310) has tabs for game data,
position, material and more. The field names below follow its game-data tab,
dialog resource **311** in the reference binary. Earlier versions of this note
called it 1031, which is the resource's language (German), not its number.
Nothing here reads or writes the reference app's own search files (Search Booster
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

### Quick filter expressions

The compact game-list filter uses the same indexed, parallel game-data scan. It
accepts up to 16 terms. Ordinary words still match names, tournament or site,
annotator, source, ECO, year or result; separate terms combine with AND.

| Example | Meaning |
|---|---|
| `Kasparov- 1985` | White player name contains Kasparov, in 1985 |
| `-Karpov B10-B17` | Black player name contains Karpov, ECO B10 through B17 |
| `Event:"World Championship"` | Event contains the quoted phrase |
| `Site:London` | Site contains London |
| `2001-2011` | Inclusive year range |
| `>2400`, `<=2200` | Average of both players' Elo values; both ratings required |
| `!1/2`, `!Site:London` | Exclude a result or a site |
| `1-0 0-1` | Either decisive result; positive result terms form one OR group |

`White:` and `Black:` are alternatives to the name suffix and prefix. Quotes
keep spaces within any term. Empty values, reversed ranges, unclosed quotes,
invalid rating comparisons and more than 16 terms return an `invalidQuery`
error; no terms are discarded. Negated terms combine with the other criteria.
The quick filter remains a whole-database search, and it can be combined with
criteria already set in the full search mask.

**Native evidence and boundary.** REA 6.1 with Ghidra 12.1.4 traced the
`Filter List` string in the supplied `CBase18.exe` (SHA-256
`8d633b10ad97bd9f8fbbc33a89c279397850ca759c8375093deda4a9485e8dee`):
string `0x14120d160`, references at `0x1408af9c4`, `0x1408af9cb`,
`0x1408b1bc5` and `0x1408b1bcc`. The shipped command resources identify
`M_KC_FILTERLIST` (489), `M_KC_FILTER_BY_NAME` (640),
`M_KC_FILTER_BY_BLACK` (642) and `M_KC_FILTER_BY_WHITE` (643). This supports
the presence of list filtering and colour-specific player filters. It does
not establish an expression parser or its grammar in that executable; the
quick syntax above is the workbench's own design.

**Difference.** In dialog 311, "&Difference:" is an auto-checkbox (control
1079, style `0x50010003`) beside one number field (1023) with an up-down
(1024), in the column of the upper Elo bound. The workbench reads it as a
maximum rating difference. This reading follows the layout and was not
confirmed in the reference app's code. The tab has no source field.

**Final position and text.** Mate, Stalem. and Check are controls 1074–1076;
Full Text and NOT are 1022 and 1081. A game is read only once it passes every
other field:
* for the final position, its move tree is decoded and the main line's end
  is tested;
* for text, only its annotation record is read.

Two things were not traced: whether the reference app reads the main line's end or a
stored flag, and whether its Check includes mate. The workbench's choices are
in the table above.

**Not ported from dialog 311:**
* Text with Good and Beautiful (1073, 2338, 2339): their meaning was not
  traced.
* In Repertoire (1897): it needs a repertoire database.
* Time Controls All/Slow/Rapid/Blitz (1315–1319): the game type is not read
  from the records.

## Annotations and Medals tabs (2026-10-16)

The reference app's search mask (dialog 310) has, besides game data and position,
an **Annotations** tab (dialog 343, class `CAnnoMaskDialog`, vtable
`0x1411e1b58`) and a **Medals** tab (dialog 312). `Search text`
(`GENERALSEARCH`, kc 597, no key) has no traced action. The workbench opens
the search mask on the Annotations tab for it (database window, Home ›
Search games).

**Annotations.** The data exchange (`0x1407d5530`) and the labels set in
the init (`0x1407d5c50`, messages 490–501, 723):

| Control | Field | Workbench |
|---|---|---|
| Text1 (1109), Text2 (1112) | 0x13000, 0x13008 | each must be in a text annotation, case-insensitively |
| Whole word (1110) | 0x12ff4 | no letter or digit on either side |
| Symbols (1114) | 0x12ff8, a text field | the symbol palette's symbols as checkboxes; each checked symbol must be in the game |
| Any text (1064), Any symbols (1065) | 0x12fc4, 0x12fc0 | the game has a text or a symbol annotation |
| Colours (1058) | 0x12fc8 | coloured squares or arrows (types 0x04, 0x05, 0x84, 0x85) |
| Training (1059), Multimedia (1060) | 0x12fec, 0x12fe0 | types 0x09; 0x10, 0x11, 0x12 (audio, picture, video) |
| Pawn structure (1061), Piece path (1062) | 0x12fe4, 0x12fe8 | types 0x14, 0x15 |
| Critical opening/middlegame/endgame position (1066–1068) | 0x12fd4, 0x12fd0, 0x12fcc | type 0x18 with kind 1, 2 or 3 |
| Variations (1063) | 0x12ff0 | the game has a variation (its moves are read) |

The critical position's kind is the byte the three commands store:
`AnnoCriticalPosOpeningCmd` (`0x1408e0cb0`) passes 1, the middlegame command
(`0x1408e0c50`) 2, and the endgame command (`0x1408e0bf0`) 3. The reader now
keeps it (`Kind::Critical`).

**Medals.** Dialog 312 has one checkbox per medal, in bit order (see
[formats](formats.md)). A game matches when its medal word has any of the
checked bits. This is read from the entry index, without reading the game.

**Combining.** Every criterion set must hold, as for the game-data tab. How
The reference app combines its criteria (any or all symbols, any or all medals) was
not traced; these are the workbench's choices. As with Full Text, only the
annotation record of a game that passes the other fields is read.

**Not ported:** Deleted (1115, 0x12fd8) and Position (1054, 0x12fdc),
whose meaning was not traced, and the Chess960 number (2380, 0x13018).

**Measurements** (classic test database, 11,151,119 entries, release build, warm,
`cbh-reader/examples/search.rs`):

| Search | Hits | Time |
|---|---|---|
| Medal Best game | 318 | 0.92 s |
| User medal | 224 | 0.93 s |
| Critical endgame position | 2,166 | 1.32 s |
| Symbol !! | 11,188 | 1.15 s |
| Text "novelty", whole word | 3,725 | 1.16 s |
| Colours | 48,212 | 1.23 s |
| Training | 2,304 | 1.28 s |


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

The search mask's Position tab is the reference app's pattern search: three boards,
mirroring and a move window. See [position search](position-search.md). This
section covers Find Position, and the exact search that the tab shares with
it.

Find Position (game window, Find Position › Hard Disk) looks for the board
and side to move of the current position. Castling rights, en passant and
move numbers are ignored, as in the reference app. It looks through every game,
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

## Measurements (classic test database, 2026-10-14)

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

On the 2CBH test database a full Find Position went from 2.73 s to 0.65 s on one
worker, and to 0.17 s on eight.
