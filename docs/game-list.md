# Game list: scrolling and sorting

Status: in progress on branch `game-list` (2026-10-13). The direction was agreed
in conversation: scroll the whole list without pages, and sort by any column
using a sort order the workbench keeps in its data folder.

## What ChessBase does

Evidence is from CBase18.exe 18.1 (x64).

* **Sorting is a view.** `0x1402cb120` sorts the list window's items by a
  column index. It is bounded by the column count at `this+0xc0` and runs a
  comparison sort through the list's virtual sort call (`vtable+0x40`). Lists
  of more than 1,000 items get a progress window. Every 128 comparisons,
  `0x1402ca5c0` pumps messages and checks for an abort. On abort it shows
  "Sorting aborted..." (`DBListItem.cpp`, `.rdata` `0x141121158`) and resets
  the sort column to −1. The database is not changed.
* **Fix Sort Order** (`FIXSORTORDER`, "Arrange games in database according to
  current sort mode", message 50876) writes that order into the database.
  So does Sort Database (`SORTDBEXPL`, "Sort database chronological by
  tournaments"). Both rewrite the files. The workbench does not port either
  yet, because sources stay read-only.
* The physical sort `0x140b14790` is an external merge sort. It uses "temp."
  run files beside the database, merges five runs at a time, and checks the
  free disk space first. Its progress messages are "Generate sort order"
  (2659) and "Final sort" (2697). Its records are 1,120 bytes per game.
* The per-database `.ini` stores the list layout as `GameListLayout<n>`
  entries. Each holds 41 column slots as visible flag, column id and width.
  No sort column is stored there.

## Scrolling

The list spans every row of the current list, whatever its size:

* the whole database in entry order;
* a search result;
* either of these sorted by a column.

The frontend renders only the visible rows. It reads rows in blocks of 100
through `read_rows(database, start, limit)`, keeps at most 60 blocks, and
drops the furthest first. The list does not use the browser's native
scrolling, because an 11-million-row element would exceed browsers' height
limits. It keeps its own first visible row, takes wheel, keyboard and
scrollbar input, and draws a scrollbar whose thumb maps to the row number.
Rows that are not loaded yet show as empty placeholders until their block
arrives. Requests for blocks that have scrolled away are not applied.

* **Go to entry** asks `find_row(database, entry)` for the entry's row in the
  current list and scrolls there. An entry missing from a search result is
  loaded without scrolling.
* **Load next/previous game** steps through the list's rows, not entry
  numbers, so it follows a search or a sort.
* **Live preview** is unchanged. A position search still opens each game at
  the node found.

## Sorting

Clicking a column header sorts by it. A second click on the same header
reverses the order, and a third returns to entry order. The `#` header always
returns to entry order. Sorting needs the column's *sort order*, a file in the
app's data folder (`sort-orders/<database hash>.<column>.cwsort`). If there is
no current one it is built first, with progress, and can be cancelled. The
source database is never written.

### Keys

Every key is an unsigned number. Entries with equal keys keep entry order.
Entries that are not games sort first, with key 0.

| Column | Key |
|---|---|
| White, Black | Rank of the player's name in alphabetical order (last name, then first name) |
| Elo W, Elo B | The rating; unrated is 0 |
| Result | 1-0, ½-½, 0-1, then the forfeits +:-, =:=, -:+, then 0-0 and lines |
| ECO | The packed ECO code with its sub-classification |
| Tournament | Rank of the tournament's name and site, then round and subround, so a tournament's games stay in round order |
| Round | Round, then subround |
| Date | The packed date: year, month, day; unknown parts are 0 |
| Annotator, Source | Rank of the name |

**Names.** Names are compared case-insensitively. Common Latin accents are
removed first ("Ánand" sorts with "Anand"), and "ß" counts as "ss". ChessBase's
own collation was not traced.

**Descending order.** It is the ascending order reversed, so equal keys
appear in reverse entry order.

### Building

1. Read the name table of the column, if it has one. Sort the names and give
   each id its rank. Equal names share a rank.
2. Read the entry index in parallel: up to 8 threads, each with its own read
   handle, take one contiguous part. Each record's key fields are read
   straight from its bytes, at the offsets the record parsers use. No game is
   decoded. Each thread sorts its part.
3. Merge the parts and write the entry numbers in order to a temporary file,
   then rename it into place. A cancelled build leaves nothing behind.

The file has a 56-byte header followed by one LE32 entry number per entry.
The header holds the magic `CWSORT01`, the column, the key version, the entry
count and the database fingerprint (size and time stamps of the entry index
and the move file). The file is not used once those change. When the
workbench saves into a database, it drops that database's sort, and the list
sorts again, which rebuilds the file.

**A changed name table goes unnoticed.** If ChessBase renames a player
without touching the entry index, the order is not rebuilt. Rows still show
the current names, but they may be out of order.

### A search result in a sorted list

The sorted search result is the sort order filtered to the result's entries.
It is computed once per result and kept while the result is.

### Bounds

* **Memory.** 16 bytes per entry while building (key and entry number). At most
  32 Mi entries (512 MB) are sorted; larger databases are refused with a
  message.
* **Disk.** 4 bytes per entry for each sorted column. At most 12 sort orders
  are kept across databases; the oldest are deleted first.
* **Reads.** Rows come from 4-byte reads of the sort order. Finding an entry's
  row reads the order sequentially until it finds the entry.

### Measurements

Release build on the development Mac (10 cores, 16 GB; 8 threads). The
example `cbh-reader/examples/sort.rs` was run on the private Mega Database
2025.

| Database | Entries | Column | Build (warm) | Peak memory | File |
|---|---|---|---|---|---|
| Mega 2025, 2CBH family | 24,783 | White | 0.26 s | 8 MB | 99 KB |
| Mega 2025, 2CBH family | 24,783 | Date | 0.06 s | 8 MB | 99 KB |
| Mega 2025, classic | 11,151,119 | White | 0.84 s | 220 MB | 44.6 MB |
| Mega 2025, classic | 11,151,119 | Date | 0.33 s | 188 MB | 44.6 MB |
| Mega 2025, classic | 11,151,119 | Tournament | 0.67 s | 189 MB | 44.6 MB |

**Cold cache.** The first run of the example took 44 s in all, almost
entirely waiting on the disk while it opened the files and resolved the names
of the rows it printed. The build itself took 0.84 s of that.

**Lookups.** Finding the row of an entry in the middle of the classic order
took 5–20 ms.
