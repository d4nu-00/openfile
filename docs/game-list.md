# Database window: game list, players, tournaments, crosstables, copy and paste

Status: in progress on branch `game-list` (2026-10-13). The direction was agreed
in conversation: scroll the whole list without pages, and sort by any column
using a sort order the workbench keeps in its data folder.

## What the reference app does

Evidence is from the reference binary (x64).

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
  tournaments"). Both rewrite the files. Neither is implemented yet. OpenFile
  supports selected 2CBH writes, but physical sorting needs its own safe
  transaction, entry remapping and cache invalidation. Current list sorting
  remains a read-only view.
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
removed first ("Ánand" sorts with "Anand"), and "ß" counts as "ss". The reference app's
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

**A changed name table goes unnoticed.** If the reference app renames a player
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
example `cbh-reader/examples/sort.rs` was run on the private test database
2025.

| Database | Entries | Column | Build (warm) | Peak memory | File |
|---|---|---|---|---|---|
| The test database, 2CBH family | 24,783 | White | 0.26 s | 8 MB | 99 KB |
| The test database, 2CBH family | 24,783 | Date | 0.06 s | 8 MB | 99 KB |
| The test database, classic | 11,151,119 | White | 0.84 s | 220 MB | 44.6 MB |
| The test database, classic | 11,151,119 | Date | 0.33 s | 188 MB | 44.6 MB |
| The test database, classic | 11,151,119 | Tournament | 0.67 s | 189 MB | 44.6 MB |

**Cold cache.** The first run of the example took 44 s in all, almost
entirely waiting on the disk while it opened the files and resolved the names
of the rows it printed. The build itself took 0.84 s of that.

**Lookups.** Finding the row of an entry in the middle of the classic order
took 5–20 ms.

## Players and tournaments panes

The database window's list has three tabs:

| Tab | Command | Default key |
|---|---|---|
| Games | `TOGGLEGAMELIST` | Ctrl+Alt+G |
| Players | `TOGGLETOURNPLAYERS` | Ctrl+Alt+P |
| Tournaments | `TOGGLEPANETOURNAMENTS` | Ctrl+Alt+T |

The reference app shows these as panes of the database window.

**What a tab shows.** The Players and Tournaments tabs list every name that
occurs in a game, alphabetically, using the collation of the sort orders.
Each name shows its number of games and the first and last year it occurs.
Deleted games are counted. A tournament also shows its site, since many
events share a name ("London" appears once per year).

**Finding and opening.**
* Typing in the find field jumps to the first name at or after the text.
  Players are typed as "Last, First".
* Double-click or Enter opens the name's games in the Games tab, as a search
  result. So does Games of Player (`VIEW_GAMES_OF_PLAYER`).
* A player's games are matched by player id, in either colour; a
  tournament's by tournament id. Neither is a name prefix.

**Building a list.** A list is the workbench's own file in the app's data
folder (`name-lists/<database hash>.<kind>.cwnames`). Each row is 12 bytes:
id, games, first and last year. The file carries the database's fingerprint,
like the sort orders.
1. The name table is read and sorted.
2. One parallel scan of the entry index counts each id's games and years.
   Each worker counts into its own table, 12 bytes per id. All workers
   together stay within 512 MB, so fewer workers run on very large name
   tables.
3. Name tables with more than 16 M ids are refused.

Saving into a database drops its lists; they are prepared again when shown.

## Crosstable

Cross Table (`DBTOOLSCROSSTABLE`, T) shows the crosstable of the tournament
chosen in the Tournaments tab, or else of the loaded game's tournament. Show
Crosstable (`SHOWCROSSTABLE`, game window) shows the loaded game's.

**Which games count.** The tournament's games are found by a search on its
tournament id, at most 5,000 games and 500 players. They are ordered by round.
When the database has a current sort order by Tournament (built by sorting
the list by that column), the games are found in it instead: the order keeps
a tournament's games together, so a binary search by the tournament's name
and site finds them, and only entries of that name are read.
* Deleted games are left out.
* Games without a result (lines, "0-0") are counted separately and not
  scored.
* Forfeits score as their result and are marked "+", "=" and "-".

**Layout.** A field of at most 24 players that played at least half of all
pairings is a round robin: one column per opponent, results from the row
player's side. Otherwise the table is Swiss: one column per round, giving the
opponent's place, colour and result (e.g. "12w1"). Either layout can be
chosen.
* The Round robin button corresponds to the reference app's `FORCEROUNDROBBIN`.
* Each result opens its game.

**Columns.** Rank, name, Elo, the results, points, games, wins, two
tiebreaks and performance.
* **Elo** is the player's first rating in round order.
* **Tiebreaks.** A round robin shows Sonneborn-Berger, then Buchholz. A Swiss
  table shows Buchholz, then Sonneborn-Berger. Players are ranked by points,
  then by those two tiebreaks, then by wins.
* **Performance** is the rated opponents' average plus the FIDE difference
  for the score. Forfeits are left out.
* **Heading.** It gives the average Elo and the FIDE category; category 1 is
  2251–2275.

**Evidence.**
* The tiebreak names are the reference app's. Its tiebreak table (`TiebreakType.cpp`,
  `.rdata` around file offset `0x110f010`) lists:
  * Buchholz, Sum of Buchholz;
  * Median1/2 Buchholz, Cut1/2 Buchholz;
  * Progressive score;
  * Sonneborn-Berger (Swiss), Sonneborn-Berger;
  * Number of wins, Number of black wins, Number of black games;
  * Direct encounter, Koya.

  Their abbreviations follow, among them "SB", "#W", "#BW", "#BG" and "KY".
* The messages include "Choose tiebreak" (50954), "Sort table by result"
  (50956), "Sort table by playername" (50958), "Category" (1096) and
  "Average Elo" (2556).
* The crosstable window is drawn by code, not built from a dialog
  template, so its exact columns were not recovered. The workbench offers
  only the tiebreaks above. Choosing among the others is not implemented
  yet.

**Measurements.** On the classic test database (release, warm cache):

| Step | Result |
|---|---|
| Player list | 463,258 names in 0.58 s, 5.6 MB |
| Tournament list | 105,313 names in 0.13 s, 1.3 MB |
| Finding a name | under 1 ms |
| Crosstable of Tata Steel TOP 81st (10 players, 45 games) | 3.5 s, almost all of it the tournament-id search over 11.15 M entries |
| The same through a sort order by Tournament (`cbh-reader/examples/crosstable.rs`) | 3.5 s → 4–9 ms for an 18-player event, 46 ms for one of 174 players; the same tables |

## Several databases, selection, copy and paste

**Several databases.** Up to four databases can be open at once, each with
its own tab in the title strip. Opening a database leaves the others, and
the loaded game, open.
* Each database keeps its own search, sort and scroll position while
  another is shown.
* A tab's × closes that database.
* The backend still refuses to open the same files for reading and editing
  at once ("This database is already open; close it first").

**Selection.** Selection follows the reference app's database window:
* a click selects one game;
* Ctrl-click (⌘ on macOS) adds or removes a game;
* Shift-click extends to the game from the last one chosen;
* Select All (`DBEDITSELECTALL`, Ctrl+A) selects the whole list.

A selection is stored as ranges of rows. Selecting all 11 M rows of the
classic test database costs nothing until a command uses it.

**Copy** (`DBEDITCOPY`, Ctrl+C) resolves the selection, or else the chosen
row, to entry numbers. It follows the list's current order, search or sort,
and remembers them, at most 50,000.

**Paste** (`DBEDITPASTE`, Ctrl+V) appends the copied games to the active
database. It works only with a .2cbh database opened for editing.
* Each game is loaded from the source and written in batches of 200, each
  synced once. The paste shows progress and can be cancelled between
  batches.
* Before it starts, the user agrees that annotations not decoded, or not
  storable by the reference app, are left out. The result counts the games affected.
* Entries without readable moves are skipped and counted.
* Pasting into the database the games came from is refused.
* The target's opening tree records every pasted game in its journal and
  stays current. Only a paste that would take the journal past 10,000
  entries makes it stale; the status then says it needs rebuilding.

**Results PGN cannot express.** PGN has no forfeits or "0-0". When a game is
loaded, a stored result of +:-, =:=, -:+ or 0-0 is kept in a `CBResult` tag.
The writer stores that code again while the PGN Result still agrees with it,
so a copied or saved game keeps its result. If the user edits Result, the
plain result wins. PGN exports carry the tag.

**Delete** (`DBEDITDELETE`, Delete) marks the selected games deleted. If all
of them are deleted already, it undeletes them, as the single-game command
does. It works only in a database opened for editing, on at most 50,000
games at once.

**Mark Games** (`MARKGAME`, +) toggles the user medal of every selected
game, as the reference app does (`MarkGamesCmd` → `0x1408d6140`, which calls
`0x14013d510` for each game: medal word XOR `0x8000`).
* A marked game shows a cyan dot beside its number. The reference app calls it "a
  user medal in cyan color" (50884).
* The status repeats the reference app's message 50885, "%d Games have been marked
  or unmarked".
* Medals are kept by Replace and travel with Paste and Convert to 2CBH.

**Edit Game Data** (`DBTOOLSEDIT`, F2 in the list) sets the tournament
(event and site), the annotator or the source of every selected game.
* The reference app's Set Tournament, Set Annotator and Set Source
  (`EditTournamentCmd`, `EditAnnotatorCmd`, `EditSourceCmd`) open their
  dialog with the first selected game's value. On OK they set that name for
  the whole selection: `InGridGameListWin` slots `0x420`, `0x440` and
  `0x430`, through `0x140bd1580` with the name kind (2 tournament, 8
  annotator).
* The workbench opens one dialog with a tick for each field, starting with
  the first selected game's values. An empty field clears the name.
* Only the records' name ids are rewritten. Names are reused or added as
  when saving, and moves and annotations are not touched.
* A new tournament gets the first selected game's date.
* The reference app's tournament details (type, place, category) and Set Game Title
  (the 2LID titles, not read) are not ported.

Both work only in a database opened for editing. They handle at most 50,000
games at once, written in chunks of 2,000 with progress, and Stop between
chunks. The selection stays after Mark Games, and after Edit Game Data
unless the list is sorted.

**Measurements.** Games of the classic test database were copied into a new 2CBH file
in the scratch folder, the same way Paste does, by
`cbh-reader/examples/copy_games.rs`:
* 2,000 games in 1.0 s and 20,000 in 9.9 s, about 0.5 ms per game (warm
  cache), split roughly 55 % loading and 45 % writing;
* every copy was read back and compared: names, event, result and the
  number of move-tree nodes all matched;
* the first comparison found one "0-0" game that came back as "Line", which
  led to the `CBResult` tag above;
* a cold first run took 26 ms per game, waiting on the disk.

## Medals

A game with any of the reference app's other fifteen medals shows an amber dot
after its number. Its tooltip names them, e.g. "Medals: Novelty, Endgame",
in bit order (see [formats](formats.md) for the traced bits). The user
medal keeps its cyan dot. Medals other than the user medal are shown only;
setting them is not ported.
