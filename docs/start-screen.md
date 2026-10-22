# Database start screen

Status: four views and local properties implemented (2026-10-22).

## What the reference app does

Evidence: the reference binary (x64), its command table, messages, ribbon and
strings.

* **My Databases** (`DBFavorites`; ribbon Home › Database, message 50002).
  The database window shows database *symbols*: a database's icon, name and
  size. "Open a database and add symbol to 'My the reference app's" (50592), so
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

**Not traced:** the formats of `Databases.cbrecent` and of the favourites.
The property storage is now established below. The earlier suggestion that
`Usage` might mean database type was wrong: it is an access counter.

## Workbench

The reference app's bitmaps are not used. The icons are the workbench's own,
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
  (the reference app's 20 types), Open Folder Location (workbench).
* **Views**: Tile (icon beside name, count, folder and description), Details
  (table, Ctrl+D), Symbols (large icon grid, Ctrl+I), and Small Symbols
  (compact rows). All share selection by path, arrows, Home/End, Enter and
  double-click opening. Switching layouts preserves selection and keyboard
  focus; changing groups always leaves a reachable first item. Missing files
  remain selectable for Properties, but cannot be opened. The last chosen
  view is saved globally in OpenFile settings and restored after restart.

**Properties** (Alt+Enter or the selected symbol's Properties button) edits
the display name, one of the existing 20 independently drawn type icons,
and a freeform description. Descriptions appear in Tile and Details and in
symbol tooltips. Saving works even when the files are missing; errors retain
the inputs. These are local preferences in `settings.json`, keyed by absolute
path, not database-file metadata. No companion or ChessBase `.ini` is changed.
There are at most 200 property records, with a nonempty name of at most 128
UTF-8 bytes and a description of at most 2,048 bytes; settings remain bounded
to 256 KiB. Moving a database does not migrate these path-bound preferences.

**Lists.** They are kept in the app's settings file (`settings.json` in its
config folder), never beside a database:

| List | Most | Order |
|---|---|---|
| Recent databases | 16 | latest first |
| My Databases (path and type) | 200 | as added |
| Folder shortcuts | 50 | as added |

* Opening or creating a database puts it first in Recent and, as in
  the reference app, adds it to My Databases unless it is there already.
* Paths are absolute and kept once each.
* Options does not change the lists: they are changed only through their
  own commands, which read, change and save the file in one step.

**Bounds.**
* The size shown is the entry count from the header record alone (46 or
  192 bytes). No database is opened to draw the screen.
* A folder shortcut lists the `.cbh` and `.2cbh` files directly in that
  folder: no subfolders, at most 500 files.

## Fresh REA/Ghidra evidence (2026-10-22)

The requested x64 `CBase18.exe` has SHA-256
`8d633b10ad97bd9f8fbbc33a89c279397850ca759c8375093deda4a9485e8dee`.
A fresh REA session with completed Ghidra analysis retained requests and
responses under `analysis/database-maintenance-20261022/` in the separate
research workspace. Addresses are virtual addresses in that executable.
No proprietary code, dialog resource or artwork is distributed here.

| Command | Actual run handler and operation |
|---|---|
| Details, key id 679 | `0x14089ac80` → `0x140720420` → `0x140729570`, list report view and font change |
| Small Symbols, 681 | `0x14089aca0` → `0x140720440`, small symbols then arrange icons |
| Symbols, 680 | `0x14089acc0` → `0x140720460` → `0x140729500`, symbols, refresh and arrange |
| Tile, 683 | `0x14089ace0` → `0x140720480`, image-size setup `0x140729480`, tile setup `0x1407295e0`, arrange |
| Properties, 675 | `0x140898e30` → selected symbol `0x140886880` → `0x140886670` → modal `0x1406b7a80` |

The functions `0x1408973c0`, `0x1408973e0`, `0x1408973f0` and
`0x140897410` are checked-state predicates, not the view operations.
Properties constructs `CDBPropertiesDia` (template 1728, vtable
`0x1411bea60`). Data exchange is `0x1406b78b0`, initialization
`0x1406b9520`, and commit `0x1406b9b90`. Controls include type list 1252,
name 1307, disabled path 2374, Training 1303, Show/Reset Protocol 1304/1305,
game count 1276, keys 1310, reference 1311, repertoire White/Black 1312/1313,
Always Open Text 1308, Permanent 1309, OK 1 and Cancel 2.

Name commit `0x140a9e5b0` writes `Title`; type commit `0x140a9e710`
writes `Type`. Config opening `0x140a9c300` → `0x140a98160` chooses the
database's same-basename `.ini`; `0x140b026f0` selects `DescrCBG` for
classic type 1 and `Descr2CBG` for type `0x1c`. `Usage` is read by
`0x140a98930` and incremented/written by `0x140a9a690`. After commit,
`0x140886670` refreshes matching symbols through `0x1408a2fd0` and
emits app event `0x89`.

**Deliberate differences and gaps.** OpenFile uses local settings rather
than importing or writing these `.ini` fields. Description is an OpenFile
addition; no native freeform description control was established. The modern
native type list has more choices than OpenFile's existing 20. Native view
setting persistence, exact icon metrics and Windows runtime appearance remain
unverified. A discovered `IconSize` string alone does not establish its storage
contract. No Windows runtime parity is claimed.

## Native UI evidence

All screenshots were captured and visually inspected in the actual native
Tauri app, using synthetic fixtures and separate validation settings:
[Tile](media/database-tile.png), [Details](media/database-details.png),
[Symbols](media/database-symbols.png),
[Small Symbols](media/database-small-symbols.png), and
[Properties for a missing database](media/database-properties.png).
Selection, keyboard opening, missing-file refusal, property save/reopen and
view restoration after app restart were exercised.
