# Database start screen

Status: spec and implementation on branch `start-screen` (2026-10-14).

## What ChessBase does

Evidence: CBase18.exe 18.1 (x64), its command table, messages, ribbon and
strings.

* **My Databases** (`DBFavorites`; ribbon Home › Database, message 50002).
  The database window shows database *symbols*: a database's icon, name and
  size. "Open a database and add symbol to 'My ChessBase'" (50592), so
  opening a database also adds its symbol. **Remove Database Symbol** (50487,
  ribbon Home › Copy) takes a symbol away and keeps the files (50488). "Pin
  to favorites" is message 4208.
* **Folders** (`TOGGLE_FOLDERTREE`, 50475 group; message 1964) shows or hides
  the folder tree and drives ("Drives", 50015).
* **Add Folder Shortcut** (`ADDFOLDERSHORTCUT`, 50475/50476) adds "a shortcut
  to a windows folder to your favorite databases".
* **Recent Databases** (1353, 2474). They are kept by `CBaseRecentManager`
  in `Databases.cbrecent`.
* **Views**: Tile, Details, Symbols and Small Symbols (`VIEW_TILE`,
  `VIEWREPORTEXPL` Ctrl+D, `VIEWSYMBOLSEXPL` Ctrl+I,
  `VIEWSMALLSYMBOLSEXPL`), and Sort Symbols (50014). "Click a database
  symbol for a preview here" (50588) describes the preview list.
* **Database types** pick a symbol's icon (strings `DBType_`, `DBTYPE`). The
  types are messages 50027–50046: Unspecific, Work, Large database, Openings,
  Informant, Classical Tournaments, Correspondence, Tactics, Analysis,
  Training, Endings, Studies, Problems, My games, Computer chess,
  Magazine/Express, Patzer, Match, Recent Tournaments and Important.

**Not traced:**
* the formats of `Databases.cbrecent` and of the favourites;
* where a database's type is stored. Its `.ini` has `[DescrCBG] Usage=`,
  which may be the type, but this was not confirmed.

## Workbench

ChessBase's bitmaps are not used. The icons are the workbench's own,
one per type.

**The screen.** The Databases workspace tab shows the start screen when no
database is open, and whenever My Databases (`DBFavorites`) is chosen.
* **Left column**: My Databases, Recent Databases, then one entry per folder
  shortcut. Folders (`TOGGLE_FOLDERTREE`) hides or shows the folder
  shortcuts.
* **Main area**: the chosen group as symbols. Each shows its icon, name,
  number of entries, format (CBH or 2CBH) and folder. A symbol whose files
  are missing is shown dimmed.
* **Opening**: double-click or Enter opens a symbol.
* **Symbol commands**: Pin to My Databases, Remove Database Symbol, Set Type
  (ChessBase's 20 types), Open Folder Location (workbench).
* **Views**: Symbols (a grid) and Details (a table, Ctrl+D). Tile and Small
  Symbols are left for later.

**Lists.** They are kept in the app's settings file (`settings.json` in its
config folder), never beside a database:

| List | Most | Order |
|---|---|---|
| Recent databases | 16 | latest first |
| My Databases (path and type) | 200 | as added |
| Folder shortcuts | 50 | as added |

* Opening or creating a database puts it first in Recent and, as in
  ChessBase, adds it to My Databases unless it is there already.
* Paths are absolute and kept once each.
* Options does not change the lists: they are changed only through their
  own commands, which read, change and save the file in one step.

**Bounds.**
* The size shown is the entry count from the header record alone (46 or
  192 bytes). No database is opened to draw the screen.
* A folder shortcut lists the `.cbh` and `.2cbh` files directly in that
  folder: no subfolders, at most 500 files.
