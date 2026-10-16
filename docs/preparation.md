# Prepare against White or Black

Status: implemented on branch `prepare-against` (2026-10-15). The direction
was agreed in conversation: build on the player statistics, the opening tree
and the ECO classification.

Evidence is from CBase18.exe 18.1 (x64), analysed statically through an REA
session (`analysis/reports/` in the research workspace).

## What ChessBase does

| Command | Id | Class, run slot | Key | Message |
|---|---|---|---|---|
| Prepare against White | `PREPARE_WHITE` (kc 585) | `PrepareWhiteCmd`, `0x1408b8030` | Ctrl+W | "Prepare against this player checking the games as White" (51020/51021) |
| Prepare against Black | `PREPARE_BLACK` (kc 586) | `PrepareBlackCmd`, `0x1408b7e40` | Ctrl+B | "Prepare against this player checking the games as Black" (51022/51023) |
| Prepare Against | `PREPARE_AGAINST` (kc 709) | `PrepareAgainstCmd`, `0x140899cf0` | none | "Prepare against a player by checking his/her games" (51109/51110) |

* **Prepare against White and Black** act on the active list window. The
  run slot takes an optional "PlayerName" parameter and calls the list's
  method at vtable offset 0x3b0. In the player list
  (`CBaseDBGridPlayerListWin`, vtable `0x1411cb1e8`) that is `0x14073b960`,
  with 0 for White and 1 for Black:
  1. It gets the chosen player's game numbers (`0x140132cb0`).
  2. It splits them into games as White and as Black (`0x140b18d80`), using
     the White and Black player ids of each game header. Deleted games (flag
     bit 0, `0x1401abcd0`) are left out.
  3. It averages the player's rating over the last ten games of the chosen
     colour (`0x1407ab590`): over the rated ones among them, by the header's
     White or Black rating getter.
  4. That average goes through a formula (`0x140464a50`): below 2850, a
     percentage 89.42282 / (((2850 − r) / 308.9042)^4.364275 + 1) + 6.76487,
     else 100, multiplied by a factor of the months since a date three years
     before the current one (`0x140464bc0`, (months / 101.7997)^0.6718763)
     and by 0.7. Results under 1000 become −1. As decompiled, the value stays
     far below 1000, so the window always gets −1. Its use was not followed.
  5. It opens a window (`0x1407a4000`) titled "Prepare against White"
     (51020) or "Prepare against Black" (51022). The window's game list is
     the games of that colour, and it loads the last of them. It is a game
     window (the cast at `0x1408e2e30`).
  6. It stores the pair {2, value} in a pane of that window (`0x14090ccc0`,
     window offset 0xee0, then 0x5c0, field 0x404). What mode 2 selects was
     not traced.
* **Prepare Against** has two parameters, PlayerName and Color, used when
  the AI assistant calls it ("Prepare against Caruana with White"). It opens
  the reference database (`0x1407b19c0`) in a database window, switches the
  window to tab 2 (`0x140a22bf0`) and, with a name, prepares against that
  player (`0x1408cdb70`). It reports "Preparation against <name> as <white
  or black> was successful." From the ribbon (database window, Home ›
  Players) it has no parameters, so it only opens the list.
* The **Dossier** (`PLAYERDOSSIER`, Ctrl+Alt+D) uses the list's slot 0x400
  (`0x14073b640`).

**Not traced:**
* what the game window does with the player's games beyond its list: mode 2
  and the value at field 0x404;
* whether the window's tree pane shows only these games.

The getters at `0x1401ceed0` and `0x1401ce870` are read as the White and
Black ratings because they sit beside the player-id getters (`0x1401ceef0`,
`0x1401ce890`) and are used the same way. This was not confirmed further.

## Workbench

**Starting it.**
* **Prepare against White** (Ctrl+W) and **Prepare against Black**
  (Ctrl+B) prepare against the player chosen in the Players list.
* Without a chosen player, they take the shown game's player of that
  colour. This is the workbench's addition, as for Player statistic, so the
  commands also work from the game window (Home › Database).
* On macOS, Ctrl+W runs from the Control key and is shown as ⌃W, since the
  system menu keeps ⌘W for Close Window. The same holds for Ctrl+Q, Ctrl+H
  and Ctrl+M.
* **Prepare Against** (database window, Home › Players) opens the Players
  list, as ChessBase's ribbon button does. With a player already chosen
  there, it prepares against them as White.

**The job** runs in the backend, cancellable from the dialog, with progress:
1. It finds the player's games with the player statistics' parallel scan
   (deleted games left out) and keeps those of the chosen colour.
2. It reads the main lines of up to 20,000 of them (the latest in entry
   order), 30 plies each, with up to 8 threads.
3. It scores them with the player statistics.

The preparation is kept per database, one at a time. It is dropped when the
database changes or closes.

**The dialog** "Prepare against White: name" shows:
* the games, years, and deleted or unreadable games;
* the score (N, +, =, −, score, Elo-Av, Perf.);
* **Recent Elo**: ChessBase's average of the last ten games' ratings with
  that colour (step 3 above). ChessBase's formula on it is not ported, since
  its use is unknown.
* **Repertoire**: the lines the player keeps playing from the start
  position, from their main lines, transpositions included.
  * A move continues a line while at least 3 % of the games (and at least
    two, or the one game of a player with one) play it, up to 16 plies.
  * At most 30 lines are shown, most played first.
  * Each has its ECO code and name from the open table (see
    [openings](openings.md)), the games, the player's score and the last
    year.
  * These thresholds are the workbench's choices; ChessBase has no such
    list.
* **Openings**: the games by the ECO code stored in each game, most played
  first, with the score (at most 20);
* **Latest games**: the last ten in entry order, with the opponent, rating,
  result, ECO and event.

**Its buttons:**
* **As White / As Black** switch the colour.
* **Show in Book** makes the Book pane answer from the player's games, as
  ChessBase's game window shows them. Its columns are the opening tree's
  (see [opening tree](opening-tree.md)). The header names the preparation,
  and **Database tree** returns to the database's opening tree.
* **List games** lists the games in the game list. Double-clicking a line
  lists the games reaching its last position.
* **Load last game** loads the last game, which is what ChessBase loads.
  Double-clicking a latest game loads it.

## Measurements

Release build, `cbh-reader/examples/prepare.rs`, classic Mega 2025
(11,151,119 entries), player A (6,925 games) with White:

| Step | Cold | Warm |
|---|---|---|
| Find the player's games (8 threads) | — | 0.12 s |
| Read 3,515 main lines (8 threads) | 21.9 s | 0.12 s |
| Repertoire (14 lines) | 0.002 s | 0.002 s |

Through the app (`src-tauri/examples/prepare_against.rs`, which adds the
scoring, names and repertoire), warm: player A with White 0.41 s (3,515
games, 14 lines), with Black 0.33 s (3,410 games, 12 lines); player B with
Black 0.29 s (4,588 games, 13 lines). The first, cold run took 54 s, waiting on
the disk.

A Book answer reads every kept line once: well under a millisecond for a few
thousand games.
