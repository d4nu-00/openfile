# Find Novelty

Status: implemented on branch `novelty-finder` (2026-10-22), stacked on
`annotation-types`. The direction was agreed in conversation.

Evidence is from the reference binary (x64), through an REA session.
Messages are cited by their ordinal in the message catalogue.

## What the reference app does

| Command | Id | Class, run slot | Key |
|---|---|---|---|
| Find Novelty (50543) | `FINDNOVELTYBOARD` (kc 477) | `FindNoveltyBoardCmd`, `0x1408e1480` | Shift+F6 |
| Novelty Annotation (51126/51127) | `FIND_NOVELTY` (kc 652) | `DBAnnotateImportantNoveltiesCmd`, `0x1408b67c0` | |

* **Find Novelty** (`0x1408f5500` → `0x1407ac5c0`), "Search the first new
  move in this game":
  * it opens the reference database (`0x1407b19c0`) and searches it for the
    game's positions (`0x14067c230`, run by `0x140a8f600`), showing an
    hourglass;
  * it walks the game's positions from the last to the first and asks the
    database's position index for each (`0x14067d190`). The first one found,
    the deepest position other games reach, gives the list of games that
    reach it (at most 8,191), as game and ply pairs;
  * at most 400 of them go to a game list window;
  * the game then selects the ply of the first result (`0x140685d70`): the
    novelty.
* **Novelty Annotation** works on the database list (`0x1402dd160` →
  `0x140405540`). It analyses each selected game of more than 20 plies with
  Tactical Analysis's machinery (`0x1403f4fd0`, `0x140416f80`,
  `0x14041cb10`), then replaces or appends it, as Centipawn Analysis does.
  Its texts ("An important novelty." 110242 and others) and its help ("A
  novelty is a reasonable engine move which has not been played in the Live
  Book or is very rare." 51422) tie it to the engine and the vendor's online
  book. Its rule was not traced.

## Workbench

* **Find Novelty** (Shift+F6; game window, Report tab) uses the open
  database as the reference database, as the Reference pane does. The game's
  own entry, when it was loaded from that database, does not count.
* It walks the main line from the start, at most 300 plies. For each
  position it asks the reference search (the opening tree, else Find
  Position; see [opening tree](opening-tree.md)) which other games reach it.
  The first position no other game reaches makes its move the novelty. That
  is the reference app's "first new move".
  * The reference app walks backwards to the deepest position other games
    reach, so a line that leaves the known games and comes back by a
    transposition can differ.
  * Asking every ply rather than halving the line matters: a halving search
    agreed with the walk on 193 of 200 test-database games. The other 7 left
    the known games and came back to them.
* **The answer** is a dialog: the novelty, how many other games reach the
  position before it, and the first 400 of them. The novelty is selected on
  the board, as in the reference app.
* A database too large to scan without an opening tree or search booster is
  not read move by move; the dialog says so. Stop ends the search.
* **Not ported:** Novelty Annotation, whose engine rule was not traced.

## Tests and measurements

* An app test on a scratch 2CBH:
  * a new game's first new move;
  * a database game whose own entry is left out;
  * a line every move of which other games played;
  * an immediate stop.
* Frontend tests: the dialog, the list, the selection and the not-searched
  message.
* Measured on the classic test database with its opening tree, release
  build, read-only and warm: 200 random entries in 19.3 s, about 0.1 s and 23
  questions per game. 6 games had no novelty because other games contain
  them entirely. Entries 11,000,000, 11,100,000 and 10,500,000 found their
  novelty at plies 17, 15 and 25.
