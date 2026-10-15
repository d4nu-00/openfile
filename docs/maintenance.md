# Database maintenance: Check Integrity, Find Double Games, Remove Deleted Games

Status: implemented on branch `maintenance` (2026-10-14). The direction was
agreed in conversation. These commands rewrite the user's own 2CBH files,
so they write to temporary files and swap them in. They never touch classic
files.

Evidence is from CBase18.exe 18.1 (x64), analysed statically through an REA
session (`analysis/maintenance/` in the research workspace).

## Check Integrity

### What ChessBase does

* **The command.** `CheckIntegrityCmd` (run slot `0x1408987b0`), "Check the
  technical integrity of all database files." (50453). It works through the
  databases chosen (`0x140159220`).
* **Before it starts** it asks:
  * "Correct errors if possible?" (2037);
  * if yes, "Critical changes will be made to your database. Do you have a
    backup?" (2038);
  * if no, "No changes will be made" (2039).
* **The checks.** Each database type has its own check: `0x140158990` for
  classic CBH (type 1) and `0x140155fa0` for 2CBH (type `0x1c`).
  * The 2CBH check (`0x140155940`) runs "Check database core" (2690,
    `0x140166840`), then the name lists (`0x14016b0b0`).
  * Then it checks the keys, "Check keys" (2692), when `.cko` and `.cpo`
    files exist.
  * It ends with "N errors" (2041) and a log line "Integritycheck <version>
    (<date>)".
* **Rebuild control structures** (4858, `0x1401576a0`) deletes `.2lcd` and
  `.2lgd` and builds them again from every game's names.

### Workbench

**Check Integrity** (Maintenance › Improve) reads every entry of the active
database. It works on any database, classic or 2CBH, read-only or opened
for editing, and it changes nothing.

* **2CBG and 2CBA headers.** The declared used length must fit the file.
* **Every record.** Its kind must be known, and the names it refers to must
  exist: players, tournaments, annotators and sources against the highest
  id of each list.
* **Every game.** The moves must decode, and the annotation record must
  parse, with the block frame, trailer and checksum checked for 2CBH.
* **Text entries** are counted, not decoded.

The scan runs in parallel through its own read handles, with progress and
Stop, like the searches. The report gives the counts, the problems by kind
and the first 500 problems with their entry numbers.

**Not ported.** Correcting errors, rebuilding `.2lcd` and `.2lgd`, the key
files, and the log line. The check reports only.

## Find Double Games

### What ChessBase does

* **The command.** `RemoveDoublesCmd` (run slot `0x14089a160`) calls
  `0x1407ab920`, "Detect double games and mark one of each pair as deleted"
  (50512).
  * It first checks that 40 bytes per game plus 2 MB of memory are free,
    asking otherwise.
  * Its dialog is `CKillDoubleDialog`, template 894 ("Dubletten löschen").
* **Settings** (section `KillDouble`, read and written by `0x1406760a0` and
  `0x140677220`, defaults from `0x1407ab920`):

  | Key | Dialog control | Default |
  |---|---|---|
  | `MinGameLen` | Game length (1287) | 0 |
  | `Decision` | Always delete first game / Always delete second game / Keep better game (1269, 1270, 1012) | 2, keep better |
  | `NameEqual` | Names: Exact / Similar / Ignore | 1, similar |
  | `TournEqual` | Tournaments: Exact / Similar / Ignore | 1 |
  | `MoveEqual` | Moves: Exact / Similar / Ignore | 1, similar |
  | `MoveCheck` | Comparison: Slow / Fast / Faster | 2 |
  | `IngoreResult`, `IngoreYear` | Ignore result, Ignore year (1286, 1285) | on, off |
  | `KeepAnnotators` | Keep different annotators (1274) | off |
  | `KillVCs` | Delete annotated games (1271) | off |
  | `MergeVCs`, `ReplaceByVCs` | Merge annotated game, Replace by VC's (1272, 1273) | off |
  | `ClipDoubles`, `RemoveDeleted` | After deleting doubles: Finish / Clip doubles / Remove deleted | — |

* **The algorithm** (`0x1406749f0`, CBaseCbgDatabase vtable `0x290`).
  1. Worker threads (`0x140671710`) build a 36-byte record per game
     (`0x140712c70`, `0x140714040`):
     * the game number;
     * a two-word key;
     * a hash of the whole move list (Exact moves), or hashes of sampled
       positions (Similar), each flagged as present;
     * the move count.
  2. **The key.**
     * The names part is a hash of the first four letters of each player's
       name (three for Similar), and is left out for Ignore.
     * With Exact moves, the move count goes into the key. With Similar
       moves and Fast, so does the ECO.
     * The result and the year are XORed in unless ignored.
     * Deleted games and text entries get keys of their own and are never
       compared.
  3. The records are sorted (`0x140716ff0`). Neighbours with equal keys
     (`0x140716e40`) are compared (`0x140713150`): equal move hashes, or any
     equal position hash for Similar.
  4. The full comparison (`0x1407136a0`) follows. Games shorter than
     `MinGameLen` are skipped, and only games of more than four plies are
     compared by position.
* **The decision** (`0x140715720`) returns the game to mark, or keeps both.
  * With Keep different annotators, two games whose annotators differ are
    both kept (record +0x30, read by `0x1401ce740`).
  * **Annotated games are preferred whatever the decision.** Of an
    annotated and a plain game, the plain one is marked. Two annotated games
    are both kept unless Delete annotated games is on.
  * Otherwise, Always first and Always second mark that game.
  * Keep better scores both games (`0x140714c90`) and marks the lower score.
    Annotations count 10. One point each goes to: the longer game, an Elo,
    both Elos, each part of the players' names, a decided result, the day,
    month and year of the date, the source, the round, the ECO and the
    tournament fields.
  * With Merge or Replace, the annotations of one game are copied into the
    other before it is marked.
* Marked games are deleted through vtable `0x150`; the list is not
  rewritten. "Write Error while deleting games" reports a failure.

### Workbench

**Find Double Games** (Maintenance › Improve) works on a 2CBH database
opened for editing, of at most 2,000,000 entries. It marks one game of each
pair deleted, at most 200,000 at once.

* **Options**, ChessBase's except where noted:
  * Decision: first, second or keep better (the default);
  * Names: Exact, Similar or Ignore;
  * Moves: Exact or Similar;
  * Game length;
  * Ignore result (on by default) and Ignore year;
  * Keep different annotators;
  * Delete annotated games.
* **The key** is a hash of the players' names (case-folded, accents
  removed), the year and the result, as the options say:
  * **Exact names** compare whole last and first names;
  * **Similar names** compare the first four letters of the last name;
  * **Ignore** leaves names out.

  With Exact moves, the number of plies is part of the key.
* **Two games with the same key are doubles when:**
  * **Exact moves:** their main lines are the same move for move;
  * **Similar moves:** also when they end in the same position (a
    transposition), or when the shorter game is a truncated copy of the
    longer. That means it has at least 10 plies, at least 80% of the
    longer game's plies, and ends where the longer game stands at that
    ply.

  ChessBase's own position samples were not traced to the ply. A first
  version that compared the positions after 20 and 40 plies paired games
  that merely shared an opening, so the workbench uses the end positions.
* **Games left out.** Games of fewer than five plies, deleted games and
  text entries are never compared.
* **The decision** follows ChessBase's rules above. "Annotated" means
  comments, symbols, coloured squares or arrows, or variations. "Keep
  better" uses ChessBase's completeness score.
* **The result** lists the pairs as "Game 3 marked; game 2 kept". A game
  kept against one double but marked against another names the game that is
  finally kept.
* **The marks are written in one write** (`Writer::set_deleted_many`),
  after every entry was checked. The opening tree records them in its
  journal, as for Delete. Remove Deleted Games then removes the marked
  games, and Delete undoes a mark.

**Not ported.** Tournaments compared (`TournEqual`), the Comparison speed,
Merge annotated game, Replace by VC's, Clip doubles, Remove deleted
afterwards, and the "First game" range. The 2 M entry limit keeps the
fingerprints within about 200 MB.

## Remove Deleted Games

### What ChessBase does

* **Commands.** "Remove Deleted Games" exists as `DBTOOLSPHYSICALDELETIONS`
  (`DBToolsPhysicalDeletionsCmd`, run slot `0x140a1fde0`) and as
  `REMOVEDELETEDGAMES` (50448, "Remove all games mark as deleted and reclaim
  disk space").
* **Disk space** (`0x14012df90`). The current database size plus 384 KB
  must be free. Otherwise ChessBase says "Not enough disk space. N MB
  needed".
* **The copy.** It then walks every entry ("Searching for deleted games",
  854). It collects the entries not marked deleted (bit 7 of the flags),
  and writes them out; cleaning up a deleted game's multimedia files is
  part of this. The job can be cancelled.

### Workbench

**Remove Deleted Games** (Maintenance › Improve) works on a 2CBH database
opened for editing, after a confirmation.

* **Lossless.** Each kept entry's 2CBG and 2CBA blocks are copied byte for
  byte, frame and all, so no annotation type is lost. A block that two
  records share is copied once.
  * Game records keep every byte except their two block offsets.
  * Text entries keep their 2CBG block. Bytes 16 onwards of a text record
    are not a 2CBA offset; in the Mega, entry 989 holds 151 there.
  * 2LID is copied unchanged, so name ids stay valid.
  * Unreferenced blocks, left behind by Replace, are dropped.
* **Refused or stopped when:**
  * an entry kind is unknown;
  * a block's marker or trailer is wrong (Check Integrity tells which);
  * the files changed during the copy;
  * the disk lacks the database's current size plus 256 MB.
* **The swap.**
  1. The new files are written as `<name>.compacting.*` and synced.
  2. The four family files, and `.2lcd`/`.2lgd` when present, are renamed
     to `<name>.compact-backup.*`.
  3. The new files are renamed into place, and the backups deleted.

  A failed rename restores the backups. Backups left by an interrupted run
  stop the next run, so they are never overwritten.
* **ChessBase's indexes** `.2lcd` and `.2lgd` are removed. ChessBase
  accepts them as absent, and its Check Integrity rebuilds them (see
  [writing](writing.md)). Other files beside the database that refer to
  game numbers are not updated, and the confirmation says so. Examples are
  ChessBase's key files (`.cko`, `.cpo`) and search booster files.
* **Afterwards:**
  * the writer and the read handles are opened again;
  * the sort orders, name lists and search results of the database are
    dropped;
  * its opening tree is deleted, since every entry number may have changed;
  * open games from it follow their new entry numbers, so Replace still
    finds the right entry. A game whose entry was removed no longer has
    one.

## Measurements

Release build on the development Mac, through
`cbh-reader/examples/maintain.rs`, warm cache.

| Job | Database | Result | Time |
|---|---|---|---|
| Check Integrity | 2CBH Mega (24,783 entries) | no problems; 24 texts | 0.18 s |
| Check Integrity | classic Mega (11,151,119 entries) | 16 problems: 14 set-up positions with impossible castling rights (e.g. entries 64687, 64740), the known Chess960 record without a start position, one annotation record that does not parse | 133.8 s (8 workers) |
| Find Double Games, defaults | 2CBH Mega | 4 doubles marked, 4 pairs kept | 0.64 s |
| Find Double Games, Exact names and moves | 2CBH Mega | none | 0.23 s |
| Remove Deleted Games | a copy of the 2CBH Mega with every tenth game marked | 2,476 removed, 22,307 kept, 39.9 → 37.9 MB, Check Integrity clean afterwards | 0.41 s |

The doubles found in the 2CBH Mega look genuine:
* Bird–Smith from two sources, with the black player's first name only on
  one;
* "Thomson" and "Thompson" against MacKenzie, twice;
* a 14-node and an 86-node version of a Lasker exhibition game.

The copy was deleted afterwards.
