# Game window: opening games, drawing on the board, game tabs

Status: opening speed and drawing are implemented on branch
`game-open-and-drawing` (2026-10-13, PR #9). Game tabs are implemented on
branch `game-tabs`, stacked on it. The direction was agreed in conversation;
the user asked to proceed without review gates.

## Opening a game

The user found that games opened slowly in the release build on the classic
Mega Database. Each part was timed before anything was changed.

### Measurements

Release build on the development Mac, classic Mega 2025 (11,151,119
entries). The example `src-tauri/examples/open_timing.rs` loads entries spread
over the whole database through the app's own registry.

| Step | Warm |
|---|---|
| `load_entry`, 399 games | 0.3 ms mean, 0.7 ms the slowest (273 nodes) |
| `load_entry`, 396 entries never read before | 0.8 ms mean, 3.7 ms the slowest |
| Serialising the response for the web view | ≤ 0.04 ms (27 KB for 273 nodes) |
| `position_at` | ≤ 0.02 ms |
| Book, start position to ply 60 | 0.5–4 ms |
| Reference search from the tree, start position (100,000 hits) | 22 ms |
| Reference search, deeper positions (tree or followed line) | ≤ 1 ms |

The web view was timed in a debug build with temporary probes (removed
again). Showing a game in the preview took about 10 ms, and drawing the game
tab about 35 ms.

None of this is slow. Three things were:

1. **The first game after a while.** The first load in a fresh process took
   7.3 s, almost all of it waiting on the disk for the first reads of the
   database's files. Later loads took about 1 ms.
2. **Game loads queued behind the list.** A game load and the list's row
   blocks shared one read handle. A block of 100 rows resolves names in
   several files, so a game load could wait behind it (about 300 ms in the
   debug build).
3. **The Reference pane's fallback.** When the opening tree cannot answer a
   position (for example any position of a game from a set-up position), the
   reference search fell back to Find Position. On the classic Mega without
   a search booster, that reads every game. It ran for **1,357 s (22.6
   minutes)**, using one core and the disk throughout, and found 42 games.
   It was cancelled only when the board moved on, and the next such position
   started another one.

A double-click also started up to three loads of the same game: one for each
click, and one to open it. Each was fast, but they queued behind each other.

### Changes

* **Own handle for games.** Each database has a second read handle, used only
  for loading games. It is opened on first use and dropped after a save,
  paste or deletion; a load that finds the files changed reopens it once.
* **Warm-up.** Right after a database opens, a background task loads its
  first game through that handle, so the first disk reads happen while the
  user looks at the list.
* **No unasked full scans.** Without a search booster, the reference search
  falls back to Find Position by itself only for databases of up to 200,000
  entries. On a larger one, the pane says that the position is not in the
  opening tree and offers **Search all games**, which can be stopped. A
  booster or a smaller database keeps the automatic fallback.
* **One load per game.** A click on the game already shown, or on the one
  still loading, loads nothing. A double-click opens the game the clicks
  loaded.

## Drawing arrows and coloured squares

### What ChessBase does

Evidence is from CBase18.exe 18.1 (x64) and its message catalogue.

* Coloured squares and arrows are stored per move as "colour commentary"
  (`ArrowListAnno` and `PreArrowListAnno` in the RTTI; CBA annotation types
  for squares and arrows, see [formats](formats.md)). Their colours are
  green, yellow and red, plus blue, cyan and orange for imported marks.
* **Delete Colour Commentary** (`ANNODELETECOLOR`, Ctrl+Alt+Y, message 1010
  "Delete all colour commentary") removes them from the game.
* In the Chess Media System, squares are highlighted "with right mouse
  button" in green, red or yellow (messages 2609–2612), and one command
  erases all arrows and highlighted squares.
* ChessBase's documented convention for graphical commentary is Alt with the
  left mouse button: a click marks a square, a drag draws an arrow. Alone it
  is green, with Shift red, with Ctrl yellow. Drawing the same mark again
  removes it.

**Not traced.** The board window's own mouse handler was not located. The
executable reads Alt (`GetKeyState(VK_MENU)`, IAT `0x1410ee7f8`) only in three
keyboard handlers (`0x140bdac94`, `0x140bdad2a`, `0x140df041a`). The board
classes `BoardWindow`, `BoardWindow2D` and `BoardWindowD2D` (vtables
`0x14123bb28`, `0x14123c490`, `0x141115788`) contain no direct modifier or
mouse-message tests. The modifier colours above are therefore the documented
convention, not a traced one.

### Workbench

* The **right button**, or **Alt with the left button**, draws on the game
  tab's board: a click marks a square, a drag draws an arrow. While dragging,
  the arrow follows the pointer. The click that ends an Alt+drag selects no
  square.
* Alone it draws green, with **Shift** red, with **Ctrl** yellow.
* Drawing a mark again in the same colour removes it; in another colour, it
  recolours it.
* The marks belong to the selected move (or to the start position). Each
  drawing is one edit (`setMarks`), so Undo takes it back. They are saved in
  2CBH and exported to PGN as `[%csl]` and `[%cal]`.
* **Delete Colour Commentary** (Ctrl+Alt+Y) clears the whole game as one
  edit.
* A position holds at most 64 coloured squares and 128 arrows; colours are
  the six PGN letters G, Y, R, B, C and O.

The preview board in the database window does not draw: it shows a game
that is not open for editing.

## Annotation symbols from the keyboard

### What ChessBase does

ChessBase 18 has no command per symbol. Its command table has no symbol
commands, and Ctrl+1 to Ctrl+9 select layouts. Instead, the game window's
key handler (`GameWin`, vtable slot 32, `0x1409e41a0`) looks at the typed
character of a key event (byte 8 of the event) and opens a popup menu of
100×300 pixels:

| Typed | Popup | Builder | Entries, in order |
|---|---|---|---|
| `!` or `?` | Move symbols | `0x140a65f60` | (none), !, ?, !?, ?!, !!, ??, Zugzwang (message 715), Only move (42) |
| `=` | Evaluations | `0x140a65aa0` | (none), +−, ±, ⩲, =, ⩱, ∓, −+, Unclear (45), Compensation (44), With attack (40), Initiative (46), Counterplay (43), Zeitnot (714), Development adv. (41), Novelty (294) |
| `$` | Prefix symbols | `0x140a66690` | (none), RR, better is (376), worse is (373), equivalent is (374), with the idea (377), aimed against (375) |

The symbol strings are UTF-16 literals in `.rdata` (`0x141269e48` onwards).
The handler only opens a popup when bit 6 of the window's state word
(`+0x4f0`) is set; its meaning was not traced. The workbench requires a
selected move.

**How the symbols are stored.** ChessBase stores three symbol slots per move,
for move, position and prefix symbols (see [formats](formats.md)). The NAG
numbers come from the Mega's 2CBA file: all 24,759 annotation blocks were
read once.

| Slot | NAGs found (count) |
|---|---|
| Move | 1 (4,049), 2 (2,665), 3 (163), 4 (424), 5 (411), 6 (584), 8 (19) |
| Position | 8 (15), 10 (137), 11 (904), 13 (164), 14–19 (495–1,516 each), 32 (4), 36 (32), 40 (89), 44 (47), 132 (29), 138 (9), 146 (32) |
| Prefix | 140 (10), 142 (778), 143 (17), 144 (2), 145 (40) |

So "Only move" is NAG 8, not 7, and "=" is mostly 11. No game has Zugzwang
(22) or "aimed against" (141). Zugzwang's slot is inferred from its place
among the move symbols.

### Workbench

* Typing **!** or **?**, **=** or **$** in a game tab opens the matching
  popup at the top of the notation. It lists ChessBase's entries in
  ChessBase's order, starting on the move's current symbol.
* **Up**, **Down**, **Home** and **End** move through the list. **Enter**
  (or a click) chooses, and **Escape** closes the popup.
* A choice replaces the move's symbol in that slot. **(none)** removes it.
  Each choice is one edit (`setNags`), so Undo takes it back.
* The keys are listed in command search as Move Symbol, Evaluation Symbol
  and Prefix Symbol. These are workbench names, since ChessBase has no
  command for them.
* They do not clash with other keys. **+** and **−** stay with the
  engine's lines, and the keys are ignored in text fields.
* The Annotation Palette (Ctrl+Alt+S) now uses the same three groups. A
  click replaces the symbol in that group, so every combination can be
  stored in 2CBH. Only move is now NAG 8, and Equal is NAG 11.

## Game tabs

### What ChessBase does

* Every game loaded from a database list opens in its own **board window**,
  until **Maximum Board Windows** are open. The option is in Options ›
  Limits (message group 50064 "Limits", item 50065), a number from 1 to 8
  (dialog builder `0x1406cd9df`: minimum at `+0xe0` = 1, maximum at `+0xe4`
  = 8). It is stored as `MaxGameWins` (key string `.rdata 0x1411da3e8`,
  read at `0x14079b6db` into field `+0xc0`), and its default is **8**
  (constructor `0x14079c01d`). The neighbouring `MaxDBListWins` ("Maximum
  Database Windows", 50066) defaults to 5.
* **Board Window** (`NEWBOARDWINDOW`, "Create a new board window", no default
  key) opens an empty board window.
* Load Next and Load Previous Game (`LOADNEXTGAME`, `LOADPREVIOUSGAME`) replace
  the game in the current board window.

How ChessBase picks the window to reuse once the maximum is reached was not
traced.

### Workbench

* **Tabs.** Each open game has a tab beside the database tabs. A tab keeps
  its own game document, selected move, unsaved-changes flag, undo history
  (in the backend document) and origin (database and entry).
* **The preview is not a tab.** Clicking a row in the database window shows
  the game in the preview. Opening it (double-click, Enter, Open game) moves
  that game into a new tab, without loading it again.
* **New tabs.** New Game, Position Setup, Paste Game, Open PGN, Board
  Window, and games opened from the Reference pane or a crosstable open a
  new tab. A game that already has a tab is shown there instead.
* **Same tab.** Load Next/Previous Game and choosing another game of a PGN
  file replace the game in the active tab, after asking about unsaved
  changes. Load Next/Previous needs the game to come from the database
  shown.
* **Maximum game tabs.** Options (`SETOPTIONS`, Ctrl+Alt+O) sets it from 1
  to 8; the default is 8, as in ChessBase. When a new tab is needed and the
  maximum is open, the game replaces the least recently used tab without
  unsaved changes. If every tab has unsaved changes, the oldest one is shown
  and the user is asked about it first. With a maximum of 1, every game
  opens in the one tab: the behaviour of a single game window.
* **Closing.** A tab's close button, or Close Game, closes the active tab
  after asking about its unsaved changes. Quitting asks about every tab with
  unsaved changes, one at a time, and stops on Cancel.
* **Following the active tab.** The book, the reference search and the engine
  follow the game shown: the active tab in the game view, the preview in the
  database window.

### State

`WorkbenchState` keeps the shown game in its existing fields (`document`,
`position`, `dirty`, `origin`), so editing, saving and analysis work on it
unchanged. `focus` names its owner: a tab id, or none for the preview. The
other games wait in `tabs` and `preview`. Changing focus stores the shown
fields into their owner, takes the target's out, and begins new game and
position requests, so a load started for one tab can never land in
another. The backend keeps up to 16 documents: 8 tabs, the preview and loads
that are still being replaced.
