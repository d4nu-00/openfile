# Game window: opening games, drawing on the board, game tabs

Status: opening speed and drawing are implemented on branch
`game-open-and-drawing` (2026-10-13). The game-tab design below is agreed
direction; its implementation follows on its own branch.

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
