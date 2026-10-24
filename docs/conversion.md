# Convert to 2CBH

Status: implemented on branch `convert-2cbh` (2026-10-14). The direction was
agreed in conversation: classic databases stay read-only, so converting them
is the way to edit their games.

For a damaged **2CBH** source, use [Recover readable games](maintenance.md#recover-readable-games).
Recovery can start before normal opening succeeds and preserves validated raw
annotation frames and metadata instead of re-encoding them through conversion.
Conversion retains its existing explicit omission policy; recovery never uses
conversion as an implicit fallback for damaged or unsupported content. Classic
recovery is not implemented.

## What the reference app does

Evidence is from the reference binary (x64), analysed statically through an REA
session (`analysis/maintenance/` in the research workspace).

* **The command.** "Convert to free 2CBH format" (`CONVERT_TO_FREE_2CBH`,
  messages 51557 and 51558) and "Convert To the reference app Format"
  (`CONVERTTOCBG`) are one command class, `ConvertToCbhCmd`. Its run slot
  (`0x140898ac0`) calls `0x14087dd30` with the target type: `0x1c` is 2CBH,
  the type code the integrity check also uses.
* **What it converts.** The databases selected on the start screen. A
  selected folder (type `0x51`) has every database in it converted, with a
  progress window "Convert %d databases to CBH/CBH2". There is no option for
  a selection of games.
* **Where it writes.** Beside the source, under the same name with the new
  extension (`DirEntry::ChangeExtension`). The database symbol's title gets
  " (2CBH)" added unless it already names the format.
  * If the target exists, the reference app says "Database exists already, games
    will be added." (51511) and appends to it (`0x1407a68f0`).
* **Disk space.** `0x1407a68f0` refuses with "Not enough disk space" (1348)
  when the free space, apparently in kB, times 2 is at most the number of
  games. That is about half a kilobyte per game.
* **Checks after copying** (`0x1407a6320`).
  * "Not all games converted" (50196) when fewer than `count − count / 500`
    games arrived.
  * For a single database it then offers "Delete old database files" (50195)
    and, on yes, deletes the source family.
* **How games are copied.** The games are appended through the target's
  import (vtable `0xb8`, with `ImportParameters`). How that import treats
  each annotation type was not traced.

## Workbench

**Convert to 2CBH** (database window, Maintenance › Maintenance) converts
the active database.

* **What it converts.** The **whole database** in entry order, or the
  **selected games** in list order. The selection is offered when more than
  one game is selected, up to 2,000,000 games. Any format can be converted:
  classic CBH or 2CBH.
* **The target** is chosen in a save dialog, named after the source. A
  family file that already exists (`.2cbh`, `.2cbg`, `.2cba`, `.2lid`,
  `.2lcd`, `.2lgd`) refuses the conversion. The workbench never appends to an
  existing database or replaces one, and it never touches the source.
  the reference app's offer to delete the source is not ported.
* **The dialog** shows the games, the target, the estimated space and the
  free space on the target disk, and what is left out:
  * annotations that are not decoded, or that 2CBH cannot store, as when
    games are pasted. Starting the conversion is the acknowledgement;
  * text entries (the writer cannot store text yet);
  * Chess960 games (the 2CBG encoding of Chess960 is not traced).

  Each of these is counted, as are entries whose moves cannot be read.
* **Deleted games stay marked deleted** (`Writer::append_marked`).
* **Progress and cancel.** The conversion shows its progress (entries done
  of all) and can be stopped between batches of 200 games. A stopped or
  failed conversion leaves nothing behind.
* **Atomic result.** The games are written into a temporary family beside
  the target (`<name>.converting.2cbh` and so on). When all are written,
  its four files are renamed to the target's names; if the target appeared
  in the meantime, the temporary files are deleted instead.
* **Afterwards** the new database opens for editing in a new database tab.
  The status line gives the counts, e.g. "49,855 games converted (145 text
  entries left out, 1,519 without some annotations)".

### Disk space

The disk had a few GB free during development, so the conversion is careful
with space:

1. **Before it starts**, it estimates the space needed (`v2::convert::
   estimate`). It refuses unless the estimate plus 256 MB fits the free
   space of the target's disk. The free space is read with `statvfs` on
   macOS and Linux (through `libc`, already in the lock file) and with
   `GetDiskFreeSpaceExW` on Windows. A disk whose free space cannot be read
   is refused.
2. **While it runs**, it reads the free space after every batch. If that
   falls under 128 MB, it stops, deletes the temporary files and says so.

**The estimate**, for *n* games:
* 192 bytes per record;
* the source's move and annotation bytes per entry, times *n*, doubled for
  classic sources;
* 372 bytes of block framing per game for classic sources;
* name groups: 0.3 per game, at most the number of names in the source,
  each 4,234 bytes.

The name file holds one group of fixed slots for each name id, so it often
dominates. The factors were measured on the classic test database (below) and
rounded up.

### Measurements

Release build on the development Mac (warm cache), through
`cbh-reader/examples/convert.rs`. The output went to the scratch folder and
was deleted afterwards.

| Slice of the classic test database | Written | Time | Written size | Estimate | Name groups |
|---|---|---|---|---|---|
| Entries 1–50,000 | 49,855 games, 145 texts | 20.4 s (0.41 ms each) | 67.0 MB | 104.8 MB (1.57×) | 6,563 |
| Entries 6,000,001–6,050,000 | 49,997 games, 3 texts | 19.7 s | 94.5 MB | 104.8 MB (1.11×) | 13,725 |
| Entries 11,000,001–11,050,000 | 49,957 games, 42 Chess960 left out | 19.5 s | 79.7 MB | 104.8 MB (1.32×) | 10,137 |
| The whole 2CBH test database (24,783 entries) | 24,759 games, 24 texts | 11.0 s | 34.3 MB | 39.9 MB (1.16×) | 3,469 |

* Per game, 2CBG took 283–310 bytes, about 134 of them block framing.
  2CBA took 233–310 bytes, mostly the empty block every game gets.
* Games without some annotations were 1,519, 40 and 1,499 in the three
  slices.
* **A whole classic test database** (11.15 M entries) would need roughly 10 GB by the
  estimate: 2.1 GB of records, about 4 GB of framing, 2.5 GB of moves and
  about 2 GB of names. It was not converted.

### Not traced or not ported

* Converting several databases or a whole folder at once.
* The " (2CBH)" title on the start screen.
* Appending to an existing database.
* Deleting the source afterwards.
* The reference app's handling of each annotation type during import.
