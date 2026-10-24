# Writing databases

This note fixes how the workbench creates and changes databases in these formats. It
covers the reference app's `NEWDATABASE`, `SAVEGAME`, `REPLACEGAME` and marking games
deleted. Reading is described in [formats](formats.md). Addresses are virtual
addresses in the reference binary, which was analysed statically and never run.

## Target format: 2CBH

The workbench writes only the newer **2CBH family**:
* It is the reference app's own family, and the reference app offers to convert databases
  to it ("Convert to free 2CBH format").
* Its reader is complete for games, and both encodings of the test database sample
  agree.
* Its name lists are fixed slots addressed by id, so there is no sorted tree
  to keep balanced. Classic CBP/CBT/CBC/CBS/CBE records sit in AVL trees, and
  those trees depend on the reference app's collation.

Classic CBH databases stay read-only. Converting them is a separate feature.

Recovery now offers a separate [verified copy](maintenance.md#recover-readable-games)
for partially readable 2CBH families. It initializes the supported writer
layout, then retains validated original raw index records, move/annotation
frames and referenced name slots. Only offsets, name ids and output counts
change. This preserves opaque bytes that normal game re-encoding cannot
represent. Recovery publishes a new directory exclusively after verification;
it does not reuse the in-place compaction transaction or write classic files.

### Files written

| File | Written | Evidence |
|---|---|---|
| `.2cbh` | Header record and one 192-byte record per game | Create path `0x14019a0e0`, setters `0x1401cef60`, `0x1401cf2c0`, `0x1401cf150` |
| `.2cbg` | 12-byte header, one block of type `0x0001` per game | `0x140198cc0`, `0x1401998d0` |
| `.2cba` | 12-byte header, one block of type `0x2000` per game, even an empty one | `0x140197be0`; all 16,178 unannotated test database games have their own empty block |
| `.2lid` | Name lists | Layout from [formats](formats.md); created by `0x1401e03e0` |
| `.2lcd`, `.2lgd` | **Not written** | See below |

**`.2lcd` and `.2lgd` are indexes over `.2lid`.**
* `.2lcd` is a paged B-tree. Its keys are lower-cased names, stored as
  byte-reversed 8-byte words.
* `.2lgd` is a `Cbg2IndexCacheFile`.
* The lib manager accepts both as absent: status 3 passes its check
  `0x140128e20`.
* The reference app's integrity check runs "Rebuild control structures"
  (`0x1401576a0`) when the `.2lcd` catalogue is empty. That routine deletes
  both files, recreates them and adds every game's names again.

This is the part most in need of a test in the real reference app.

### Headers

* **2CBH header record (192 bytes):** all zero except:
  * byte 8 = `0x26`;
  * LE16 stride 192 at byte 10;
  * byte 13 = 5;
  * LE64 record count, header included, at byte 16;
  * LE32 mode 0 at byte 36.

  This is exactly the header of the test database copy.
* **2CBG:** LE64 used length, LE16 header size 12, then LE16 1. The reference app
  writes 1 on create. The test database copy has `0x0500`; no code in 18.1 writes that
  value, and the reader checks only the header size.
* **2CBA:** LE64 used length, then LE32 12.
* **2LID:** the big-endian header from [formats](formats.md), with the six
  lists in their fixed slot sizes:

  | List | Slot size |
  |---|---:|
  | Players | 1,024 |
  | Tournaments | 1,120 |
  | Sources | 220 |
  | Annotators | 1,024 |
  | Teams | 314 |
  | Titles | 532 |

  Each list's two all-ones words, the trailing `ffffffff 00000001` and the
  `(kind, 1)` pairs are copied as observed.

### Game records

| Bytes | Written value |
|---|---|
| 0–1 | `0x0001`, or `0x0081` when marked deleted |
| 2–3 | `0x0101` |
| 8, 16 | 2CBG and 2CBA block offsets |
| 24–56 | White, black, tournament, annotator and source ids; all-ones means none |
| 64, 72 | Team ids, all-ones (teams are not edited yet) |
| 80 | LE64 2, the value of every test database game without nested annotation types (those have 3) |
| 88, 89 | Result code and line evaluation (see formats) |
| 90, 92 | Round, subround |
| 96, 112 | White and black Elo |
| 128 | Packed ECO |
| 130 | Medal word: kept by Replace, copied by Paste and conversion, toggled by Mark Games; 0 for a new game |
| 138 | Number of moves in the main line, capped at 255 as in classic byte 45 |
| 188 | Packed date |
| everything else | 0 |

The zeroed fields include:
* classic bytes 37–44 (bytes 130–137);
* the values at 140/144;
* the hash at 152;
* the FILETIME at 160.

The reference app uses them for list icons, material search and duplicate checks.
They are not needed to show or replay a game. Recording more of them is
follow-up work.

### Names

New names are appended to their list. Before appending, the writer looks for
an existing record with exactly the same strings and reuses its id:
* players: last name and first name, split at the first ", " of the PGN name;
* tournaments: site and event;
* sources: title and publisher;
* annotators: player records, as in the reference app.

Fixed record tails are copied from what every test database record holds:
* a player record ends with the 24 bytes seen in all 3,471 players;
* a tournament record ends with a 108-byte tail. It holds the packed game
  date, the value 7 at tail bytes 34, 45 and 56, and zeros elsewhere: no type,
  nation or coordinates.

Records live in group *n* of the file. The writer extends the file to whole
groups.

### Moves and annotations

* **Moves** are encoded with the reader's token table, inverted.
  * Captured-piece alternatives are chosen from the piece actually taken.
    The order is checked against every test database game before relying on it.
  * Set-up games use the placement tokens.
  * After a line's end marker, variations are written in the order the
    reader expects: later moves first, siblings in order.
* **Annotations** are written per node, in 2CBA line order, as:
  1. text before;
  2. one symbol entry (move, position and prefix NAG);
  3. text after;
  4. squares;
  5. arrows.

  This is the most common order in the sample.
  * Text is UTF-8, which the reference app detects, with flags 0 and language 7
    ("none").
  * NAGs 1–9 are move symbols, 140–145 prefixes, and everything else a
    position symbol.
* **Content the reference app cannot store** is reported and needs an explicit
  acknowledgement before saving, as PGN export already does. Examples: a
  second NAG in the same slot, a mark colour outside G Y R B C O, a null move
  as the first move of a variation.

### Operations and crash safety

All writes go through a single `Writer` that owns the open files.

* **New database:** create the four files with empty contents. The 2CBH count
  is 1 (the header). Refuse when any family file already exists.
* **Save new game**, in this order:
  1. append the 2CBG and 2CBA blocks at the used lengths and sync;
  2. update the two file headers' used lengths and sync;
  3. append any new name records, then update the 2LID counts and sync;
  4. write the game record and sync;
  5. raise the 2CBH count and sync.

  A crash leaves at most unreferenced bytes past the declared lengths. The
  next write overwrites them.
* **Replace game:** steps 1–3, then rewrite the game's record in one 192-byte
  write. The old blocks stay in the file unreferenced, as in the reference app, until
  the database is compacted.
* **Mark deleted / undelete:** rewrite byte 0 of the record.

Only one writer may hold a database. Every write first checks that the files
still have the size and modification time it last saw. A change made outside
the workbench stops the writer.

### What may be written

* A database is writable only if:
  * the user created it with New Database; or
  * the user opened it with the explicit Open Database for Editing command,
    which warns that the database's files will be changed.
* Databases opened with Open Database stay read-only, as now.
* Classic families are never writable.
* A family that aliases a database open read-only is refused (same device and
  inode, or `same-file` on Windows).
* PGN saving keeps its existing guards: never into a database family file.

### Validation

`examples/write_sample` copies a bounded, deterministic sample of test database games
into a temporary database outside the repository. It then reads them back
with `Database`, then:
* checks move trees, metadata and represented annotations against the source;
* checks that each 2CBG payload is byte-identical to the original;
* compares 2CBA payloads after decoding.

Tests use synthetic games only.

In the real reference app (Windows), please check:
1. The database opens, and the game list shows players, events, dates,
   results and ECO.
2. Games replay with their variations, comments, symbols, arrows and
   coloured squares.
3. Tools › Check integrity reports no damage, or repairs it, for example by
   rebuilding the control structures.
4. Adding a game in the reference app reuses existing player names, after the check
   above if needed.
