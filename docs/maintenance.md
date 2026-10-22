# Database maintenance and safe recovery

Status: integrity, doubles and compaction implemented; safe recovery added
2026-10-22. Integrity is read only. Doubles and compaction require a 2CBH
opened for editing. Recovery writes only a separate new result directory.
Classic families are never written.

Evidence is from the reference binary (x64), analysed statically through an REA
session (`analysis/maintenance/` in the research workspace).

## Check Integrity

### What the reference app does

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
  * Fresh tracing establishes `0x140155fa0` → `0x140162750` for the
    2CBH core check, followed by the name lists (`0x14016b0b0`). Earlier
    notes identified another core routine at `0x140155940`; that is not
    the fresh traced call chain.
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

### What the reference app does

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

* **Options**, the reference app's except where noted:
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

  the reference app's own position samples were not traced to the ply. A first
  version that compared the positions after 20 and 40 plies paired games
  that merely shared an opening, so the workbench uses the end positions.
* **Games left out.** Games of fewer than five plies, deleted games and
  text entries are never compared.
* **The decision** follows the reference app's rules above. "Annotated" means
  comments, symbols, coloured squares or arrows, or variations. "Keep
  better" uses the reference app's completeness score.
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

### What the reference app does

* **Commands.** "Remove Deleted Games" exists as `DBTOOLSPHYSICALDELETIONS`
  (`DBToolsPhysicalDeletionsCmd`, run slot `0x140a1fde0`) and as
  `REMOVEDELETEDGAMES` (50448, "Remove all games mark as deleted and reclaim
  disk space").
* **Disk space** (`0x14012df90`). The current database size plus 384 KB
  must be free. Otherwise the reference app says "Not enough disk space. N MB
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
    are not a 2CBA offset; in the test database, entry 989 holds 151 there.
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
* **The reference app's indexes** `.2lcd` and `.2lgd` are removed. The reference app
  accepts them as absent, and its Check Integrity rebuilds them (see
  [writing](writing.md)). Other files beside the database that refer to
  game numbers are not updated, and the confirmation says so. Examples are
  the reference app's key files (`.cko`, `.cpo`) and search booster files.
* **Afterwards:**
  * the writer and the read handles are opened again;
  * the sort orders, name lists and search results of the database are
    dropped;
  * its opening tree is deleted, since every entry number may have changed;
  * open games from it follow their new entry numbers, so Replace still
    finds the right entry. A game whose entry was removed no longer has
    one.

## Recover readable games

### Reference evidence and compatibility boundary

Fresh REA/Ghidra analysis used the same x64 executable and SHA-256 recorded
in [start screen](start-screen.md#fresh-reaghidra-evidence-2026-10-22).
Raw requests/responses remain in the separate research workspace under
`analysis/database-maintenance-20261022/`. String/class inventory did not
establish a standalone safe-copy recovery command. The traced repair is
**Check Integrity in place**; OpenFile's recovery command is explicitly its
own operation, with no invented native command identifier.

Direct observations from actual handlers:

* Dispatch `0x140159220` asks whether to correct errors (2037), then asks
  about a backup (2038), or states no changes (2039). It selects classic
  `0x140158990` or new-family `0x140155fa0` by family type.
* `0x140162750` validates index entries through `0x140162470` and stream
  prologs through `0x14015f010`, rebuilds name controls `0x1401576a0`,
  and checks records `0x140156370`. The index check compares declared and
  physical record counts; extra physical entries can update the count in
  repair mode. Stream repair can adjust used length; versions above `0x500`
  are refused.
* `0x140156370` walks entry ordinals, reports header read failures, and
  calls `0x140157f60` to validate each game. It checks abort/progress every
  100 entries. Repair may mark an irrecoverable entry deleted through virtual
  slot `+0x150`; recoverable adjustments reach `0x14016eb70`, load the
  entry, change it through `0x1401ae740(..., 0)` and write `0x14013ca20`.
* `0x140157f60` checks frame prolog, positive length and physical bounds,
  refuses game blocks over `0x2000000` (32 MiB), and attempts game loading.
  Frame read/write `0x1401af990` / `0x1401afc80` corroborate the existing
  [block contract](formats.md): marker, length/padding, checksum, type and
  trailer. Control rebuild deletes and recreates `.2lcd`/`.2lgd`.

The native backup implementation, atomic rollback and crash behavior were
**not established**. The executable was statically analysed, not run on
Windows. OpenFile's separate-directory publication and smaller resource
limits below are deliberate safety choices, not claimed native behavior.

### Product workflow and retention

Maintenance → **Recover readable games** is available without opening a
database. Choose a `.2cbh` source and a **new folder name that does not
exist**. Recovery creates `Recovered.2cbh`, its three required companions,
`recovery.jsonl` and `recovery-summary.json` together inside that folder.
The dialog shows recovery/verification progress, Stop, counts, reasons and
the report path. Only an explicit **Open recovered database** opens the new
copy, read only. Failure retains the source/destination choices for retry.

The source does not need to pass the normal opener. Its signature/version
and 192-byte index stride must still be established. All complete physical
records are examined, including records beyond a damaged smaller declared
count; these receive a warning. An incomplete last index record is reported
corrupt. There is no byte-by-byte search for guessed boundaries or
unreferenced stream blocks.

* Each retained game must have a readable, checksummed, correctly typed
  move frame that decodes within the reader's supported encodings and limits.
  Supported variations, setup positions, special results, deletion marks,
  ratings, dates, rounds, medals and opaque record fields are retained.
* Annotation frames must validate and parse. The entire original raw frame
  is copied, including content OpenFile cannot display; a warning identifies
  that content. No annotations are re-encoded or silently stripped. An absent
  annotation reference means no annotations; a required missing/damaged
  companion causes the entire game to be skipped.
* Referenced player, tournament, annotator, source and team slots must pass
  the established 2LID layout and bounded field checks. Whole original slots,
  including opaque tails, are copied; only ids and stream offsets are
  remapped. Unreferenced names are omitted. Missing/invalid referenced names
  cause a corrupt-entry skip rather than invented metadata.
* Text entries, unsupported move encodings/flags and per-entry resource
  limits are explicitly counted **unsupported**. Malformed/truncated data
  are **corrupt**. This delivery has no optional omission policy: marked
  deleted games are retained. Classic CBH recovery is explicitly refused.

The streamed JSON-lines report has one outcome per scanned entry with old
entry number, new entry number for recovered games and a reason for skips
or retained-content warnings. The summary also records family-level warnings
such as a damaged header. The dialog/summary retain the first 500 problems
and a truncation flag; the full entry report is not truncated.

### Publication, cancellation and bounds

All source handles are read only. Work takes place in a unique hidden
`.openfile-recovery-*` directory beside the destination, initialized through
the supported writer. Files and the report are synced, then the normal
opener and full integrity checker verify the output. Source file identities,
sizes, modification times and companion inventory are checked again before
publication. Concurrent edits are refused; this is not a filesystem snapshot
or a guarantee against a malicious writer preserving all file metadata.

An exclusive atomic directory rename publishes the whole result: macOS
`renamex_np(RENAME_EXCL)`, Linux `renameat2(RENAME_NOREPLACE)`, or Windows
directory rename, which refuses an existing destination. Existing files,
directories and symlinks are never replaced. Source/destination overlap is
refused after resolving the parent directory. A destination appearing during
verification also wins the race and is preserved.

Stop and ordinary errors remove this job's temporary directory and publish
nothing. A killed process can leave its hidden temporary directory; a retry
uses a new one and never overwrites those leftovers. A crash after the commit
rename can leave a complete verified result. Power-loss durability depends
on the filesystem; a parent-directory sync failure after publication is
reported as a warning. Existing databases, open games, selections and caches
remain bound to their original source; recovered entry remapping is recorded
in the report and opens as an independent database. No source indexes are
copied or invalidated.

| Resource | Bound |
|---|---|
| Physical candidate entries | 2,000,000; larger sources refused |
| Decoded payload / raw frame | 4 MiB / 8 MiB per block, including padding in frame limit |
| Output streams | 32 GiB each for moves and annotations; exceeding it aborts the job |
| Retained distinct names | 250,000 total; further affected entries reported unsupported |
| Copy buffer | 64 KiB; no whole-family or shared-frame cache |
| In-memory problems | 500; full outcomes streamed to disk |
| Destination disk | At least 256 MiB initially; stop below 128 MiB at progress checkpoints |

Disk checks occur every 100 candidates and during verification. They are not
a reserved quota; write errors abort and clean up. Shared source blocks are
copied for each retained game, so output can exceed source size. Reports and
name slots require additional space beyond the two stream caps. Stop is
checked between entries and before publication; a filesystem call, bounded
block decode, or verification's bounded name-table scan can delay it.

### Native acceptance and measurements (2026-10-22)

A separately identified native Tauri build used only disposable synthetic
families for writes. An 80-game original included comments, variations,
coloured squares/arrows, metadata, special results and one deletion mark.
Its damaged copy had a bad entry count, independent move/annotation/name
damage at entries 3/7/11 and a text entry at 80. Native recovery produced
76 games, three corrupt skips and one unsupported text. All 76 decoded
games plus results/deletion/medals matched the known original; the output
passed native Check Integrity (76 games, one deleted, no problems).

Native progress and Stop were exercised on 100,000 entries: no result was
published after Stop. A subsequently created destination with a marker file
was refused and preserved. Existing Check Integrity passed on the original;
Find Double Games compared 80 and marked zero (these four-ply fixtures fall
below its minimum comparison length). A separate two-game, eight-ply duplicate
fixture then marked game 2 and kept game 1; integrity reported two games, one
deleted and no problems. Compaction on another disposable copy removed one deleted
game, retained 79 and passed integrity afterwards. Large native read-only
browsing reached entry 11,151,119 of the available classic database.

Warm-cache release examples on the development Mac, including verification:

| Operation | Time | Whole-process peak RSS | Four-file output |
|---|---:|---:|---:|
| Damaged synthetic 80 → 76 games | 0.130 s (0.43 s process wall) | 3.33 MiB | 373,234 bytes |
| Synthetic 100,000 → 100,000 games | 2.635 s | 5.42 MiB | 61,608,868 bytes |
| Read-only 11,151,119-entry classic: open, final 100 rows, last game | 2.83 / 10.90 / 1.13 ms | 3.61 MiB | no writes; 10,894 bytes read |

These are small-game recovery fixtures, not a maximum-memory or worst-case
disk benchmark. Large database recovery is capped at 2 M entries; only
browsing was measured on the 11 M source. The CLI process RSS excludes the
native WebView. Item reports require extra disk space.

All native screenshots were captured and visually inspected:
[result](media/database-recovery-result.png),
[output integrity](media/database-recovery-integrity.png),
[progress](media/database-recovery-progress.png),
[stopped](media/database-recovery-stopped.png),
[destination conflict](media/database-recovery-collision.png), and
[compaction](media/database-compaction.png), and
[positive duplicate marking](media/database-doubles.png).

## Follow-on §2 work

Physical Sort Database and Fix Sort Order remain separate from existing
list-view sorting and require transactional entry remapping for open games,
sort/search indexes and opening trees. Setting ECO is already implemented;
final-material/rating/team/round bulk updates and previewable name cleanup
remain unfinished. No niche P3 features are included here.

## Earlier maintenance measurements

Release build on the development Mac, through
`cbh-reader/examples/maintain.rs`, warm cache.

| Job | Database | Result | Time |
|---|---|---|---|
| Check Integrity | 2CBH test database (24,783 entries) | no problems; 24 texts | 0.18 s |
| Check Integrity | classic test database (11,151,119 entries), before the set-up fix | 16 problems: 12 set-ups with impossible castling rights (e.g. entries 64687, 64740), two damaged move records, the known Chess960 record without a start position, one annotation record that does not parse | 133.8 s (8 workers) |
| Check Integrity | classic test database, after dropping impossible castling rights (2026-10-15) | 4 problems: the two damaged move records (3079389, absent piece ordinal; 7168922, no line terminator), the Chess960 record (489834), the annotation record (2455498) | 157.9 s (8 workers, with a Ghidra analysis running beside it) |
| Find Double Games, defaults | 2CBH test database | 4 doubles marked, 4 pairs kept | 0.64 s |
| Find Double Games, Exact names and moves | 2CBH test database | none | 0.23 s |
| Remove Deleted Games | a copy of the 2CBH test database with every tenth game marked | 2,476 removed, 22,307 kept, 39.9 → 37.9 MB, Check Integrity clean afterwards | 0.41 s |

The doubles found in the 2CBH test database look genuine:
* Bird–Smith from two sources, with the black player's first name only on
  one;
* "Thomson" and "Thompson" against MacKenzie, twice;
* a 14-node and an 86-node version of a Lasker exhibition game.

The copy was deleted afterwards.
