# Engines: kibitzers, lines, pasting and Engine Management

Status: implemented on branch `game-tabs` (2026-10-13). The direction was
agreed in conversation; the user asked to proceed without review gates.

## What the reference app does

Evidence is from the reference binary (x64), its command table and messages.

| Command | Id | Default key | Message |
|---|---|---|---|
| Add Kibitzer | `ADDKIBITZER` | Ctrl+K | "Add a Kibitz engine pane to analyse or assist in a game" |
| Remove Kibitzer | `CLOSEKIBITZER` | Ctrl+Shift+K | "Remove the last Kibitz engine pane" |
| Remove All Kibitzers | `CLOSEALLKIBITZERS` | none | "Remove all Kibitz engine panes" |
| Default Kibitzer | `TOGGLEDEFAULTKIBITZER` | Alt+F2 | "Activate or deactivate the default kibitzer" |
| Engine Management | `ENGINEMANAGEMENT` | none | "Activate and deactivate engines" |
| Increase/Decrease number of lines | `INCREASENLINES`, `DECREASENLINES` | +, − | "… the number of main lines computed by the engine" |
| Paste Engine Variation | `PASTEENGINEVARIATIONS` | Ctrl+Space | "Paste current engine analysis variation into notation" |
| Paste Engine Move | `PASTEENGINEMOVE` | Shift+Space | "Paste current best engine move into notation" |

* Each command is a class (`AddKibitzerCmd`, `CloseAllKibitzersCmd`,
  `ToggleDefaultKibitzerCmd`, `IncreaseNLinesCmd`, `DecreaseNLinesCmd`,
  `PasteEngineMoveCmd`, `PasteEngineVariationsCmd`; Engine Management's
  dialog is `CEngineManagementDia`). Their run slot (vtable slot 26:
  `0x1409c82b0`, `0x1409c8350`, `0x1409c6d20`, `0x1409c55e0`) casts the
  active frame to `CGameFrameWnd` and calls a virtual function of the object
  at its offset `0xe60`: slot `0x4c8` to paste a move, `0x4b0` to paste a
  variation, `0x5b0` to change the lines. That object's class, and so the
  exact paste rules, were not traced.
* Evaluations that engines leave in a game are a CBA annotation type of
  their own (Delete Commentary has "evaluation/time", `ANNODELETEFRITZ`). Its
  layout is not decoded yet ([formats](formats.md)).

## Workbench

### Kibitzers

* Up to **4 kibitzers** run at once, each in its own pane with its engine,
  Stop/Start, lines and options. All follow the game shown (the active tab,
  or the preview). A kibitzer starts analysing as soon as it is added.
* **Add Kibitzer** (Ctrl+K) opens Engine Management when engines are
  installed, otherwise asks for an executable; the pane's **Add kibitzer**
  button always asks for one.
* **Remove Kibitzer** (Ctrl+Shift+K) removes the pane used last;
  **Remove All Kibitzers** removes them all.
* **Default Kibitzer** (Alt+F2) starts the default engine, or removes it when
  it runs. Without a default engine it opens Engine Management.
* **Infinite Analysis** starts or stops the pane used last; it no longer has
  Alt+F2, which is the reference app's key for Default Kibitzer.

### Lines

**+** and **−** change the pane's lines, the engine's `MultiPV` option, from
1 up to the engine's maximum (at most 128). A running analysis is stopped,
the option set, and the analysis started again. An engine without `MultiPV`
says that it computes one line only.

### Pasting

* **Paste Engine Variation** (Ctrl+Space) pastes the best line (MultiPV 1)
  of the pane used last, from the current move. **Paste Engine Move**
  (Shift+Space) pastes its first move and selects it.
* Moves already in the notation are followed; the first new move becomes the
  continuation when the position has none, and a variation otherwise. The
  paste is one edit, so Undo removes it; nothing is pasted unless every move
  is legal.
* A pasted line ends with a text comment holding the score from White's
  side and the depth, e.g. `+0.35/22` or `+M3/30`. This is the workbench's
  form, chosen because the reference app's own evaluation annotation is not decoded;
  it is plain text in 2CBH and PGN.

### Engine Management

The installed engines are kept in the app's settings file
(`settings.json` in its config folder) with a name, the executable's path
and the options changed for it. **Add engine…** asks for an executable and
names it from its UCI handshake (`id name`); the first engine becomes the
default. Options changed in a kibitzer of an installed engine are kept and
applied the next time it starts. At most 32 engines are kept; paths must be
absolute.

**When an engine does not start**, the error says why where it can tell:
* A folder chosen in place of the program names the programs inside it.
* On macOS, a program downloaded with a browser carries the system's
  quarantine attribute. If it is not notarized (engine releases usually are
  not), macOS kills it as it starts, before the UCI handshake. When a start
  fails that way and the file has the attribute, the error says so and gives
  the `xattr -d com.apple.quarantine` command that allows it. The workbench
  never removes the attribute itself.

## Chess960

A position whose castling rooks stand off their standard squares is a
Chess960 position. Before analysing it, a session sets `UCI_Chess960` to
true, and sets it back to false for the next standard position. Castling
moves go to the engine king to rook. An engine without the option is refused
with a message. Chess960 games whose castling matches the standard squares
are sent as standard chess, which plays the same. Users cannot set
`UCI_Chess960` themselves.

## Syzygy tablebases

The reference app has "Use Tablebases" (message 737) and a "Tablebases" page (2554);
its own access goes through `TBAccess.dll`, which was not studied.

* The user chooses a folder of Syzygy files (`.rtbw`, `.rtbz`) in Options.
  The folder is kept in the settings file and opened in the background at
  start; a missing folder shows as no tables. At most 4,096 table files are
  read from it. No tablebase file is part of the workbench.
* Probing uses `shakmaty-syzygy` 0.28 (GPL-3.0-or-later, by the author of
  `shakmaty`, which the workbench already uses).
* The **Tablebase** tab beside Book and Reference shows, for the board:
  the result for the side to move (with the 50-move rule: "cursed" wins and
  "blessed" losses are drawn by it), the distance to the next capture or
  pawn move (DTZ, in plies), and every legal move with its result and DTZ,
  best first: wins fastest first, mates first, losses slowest first.
  Clicking a move plays it, or selects it when the game has it.
* Positions with castling rights, or with more pieces than the largest
  table, are reported as such.

### What the reference app does with tablebases

Evidence: the reference binary.
* Its Options keep Syzygy and Nalimov folders (`TablebaseOptions`,
  settings key `V_TABLEBASES`; strings "Syzygy (GUI)" and "Syzygy (Engine)").
* It hands the Syzygy folder to engines with `setoption name SyzygyPath
  value …`, and a Nalimov path the same way.
* "Use Tablebases" (737) is used by two functions (`0x140066670`,
  `0x140075690`) that were not followed.
* The search mask's Material tab (dialog 328) has a "Tablebase" checkbox
  (1090).

No the reference app command annotates a game from the tables, and how the reference app
shows tablebase results in the notation was not traced.

### Engines and the notation (workbench)

* **SyzygyPath.** Each engine that offers `SyzygyPath` is pointed at the
  chosen folder when it starts. That covers kibitzers and Tactical
  Analysis, as in the reference app. Engines without the option are left as they
  are.
* **Tablebase Annotation** (Analysis ribbon, a workbench command) reads the
  main line, at most 1,000 moves:
  * the move that brings the game into the tables gets the result, e.g.
    "Tablebase: White wins";
  * a move that gives away a win or a draw for its side gets "?", or "??"
    from a win to a loss, and the result after it, e.g. "Tablebase: draw";
  * the tables' best move becomes a variation, with its result.

  Wins drawn by the 50-move rule count as draws. It is one edit, so Undo
  removes it, and it adds nothing if the game changed meanwhile. The
  symbols and texts are the workbench's own.

## Engine search commands: threat, lock, next best

Status: implemented on branch `engine-pane-commands` (2026-10-22). The
direction was agreed in conversation.

### What the reference app does

Evidence is from the reference binary (x64), through an REA session.

| Command | Id (kc) | Class, run slot | Key |
|---|---|---|---|
| Lock Engine | `LOCKENGINE` (107) | `LockEngineCmd`, `0x1409c7800` | Ctrl+Alt+L |
| Goto Lock Position | `GOTOLOCK` (108) | `GotoLockCmd`, `0x1409c6be0` | Ctrl+L |
| Next Best | `NEXTBEST` (109) | `NextBestCmd`, `0x1409c7e50` | Y |
| Search Better | `SEARCHBETTER` (110) | `SearchBetter0Cmd`, `0x1409c8c50` | Shift+Y |
| Search Clearly Better | `SEARCHCLEARLYBETTER` (111) | `SearchBetter1Cmd`, `0x1409c8d00` | Alt+Y |
| Analyse threat | `ANALYSETHREAT` (135) | `AnalyseThreatCmd`, `0x1409c41d0` | X |

The command ids follow from the command registration (`0x1409b9640`), which
stores each class's kc after creating it.

* **The game window.** The object at the game frame's offset `0xe60`, which
  earlier notes left untraced, is a `GameWin` (vtable `0x141253370`). Its
  slots 0x5d8, 0x5e8, 0x5f8 and 0x608 are Lock, Goto Lock, Next Best and
  Search Better (`0x1409e4c60`, `0x1409e2490`, `0x1409e5580`, `0x1409e7570`).
  Each finds the active engine and calls the engine window's own function.
* **Lock Engine** (`0x140309770` → `0x14031fe80`) flips a lock flag on the
  engine (offset 0x5e3). When the lock is released and the board shows
  another position, it posts an event that brings the engine back to the board
  (`AppEvent` 0x18). **Goto Lock Position** (`0x140308f90`) shows the locked
  position, and only while the flag is set (`0x140309040`).
* **Next Best** (`0x140309080`) takes the engine's best line and adds it to
  a list of excluded lines (`0x140998280`, 0x50-byte `EvaluatedVariation`
  entries). The list is emptied when:
  * the position has changed;
  * the best move is already in it;
  * it holds as many moves as the position has legal moves.

  The engine then restarts (`0x140accf70`).
* **Search Better** (`0x140309490`) does the same and also needs the best
  line's score. It restarts the engine with a value (`0x140acc1c0`): 0 for
  Search Better, and a setting of the engine window (offset 0x24 of the object
  at 0x4f0) for Search Clearly Better.
* **The UCI command** (`0x140af7a10`). When the search has a move list (mode
  2), `go` gets ` searchmoves` and the moves. With the value above, it also
  gets ` minscore` and the score, but only for engines that announce that
  extension. Standard UCI has no such command.
* **Analyse threat** (`0x1409ecbb0` → `0x140acd030` → `0x140ac9e60`) flips a
  threat flag on the engine (offset 0x5e1). Switching it on is refused in some
  positions. One check is a virtual of the position (slot 0x218), probably
  "in check". It then tries a null move on the engine's position, and when that
  fails it sends a copy of the position with the other side to move. Switching
  it off goes back to the game's position.
* "Display old moves (Next Best)" (message 3205) is an option that shows the
  excluded moves.

**Not traced:**
* the size of Search Clearly Better's margin (the setting at offset 0x24);
* every condition that refuses a threat;
* whether the threat flag survives a move on the board;
* how the excluded moves are displayed.

### Workbench

The commands act on the kibitzer used last. Each pane also has **Threat**,
**Next best** and **Lock** buttons. The commands are in the Analysis ribbon's
"Engine search" group, with the reference app's keys.

* **Analyse threat** (X) analyses the position after a null move: the same
  pieces with the other side to move, and no en passant square. The scores
  and lines are for that side. The game's earlier moves are not sent, so the
  engine sees no repetitions before it. A side in check has no threat to
  analyse; the pane says so and goes on analysing the position. Pressing X
  again, or a move on the board, ends it. The latter is the workbench's
  choice, since the reference app's rule was not traced.
* **Next Best** (Y) leaves the current best move out and starts again with
  UCI `searchmoves` set to every other legal move. The pane lists the moves
  left out with the score each had. As in the reference app, the list is
  emptied when:
  * the board shows another position;
  * the best move is already in it;
  * every legal move would be left out.
* **Search Better** (Shift+Y) and **Search Clearly Better** (Alt+Y) also
  leave the best move out. They then say whether the new best line scores
  more than the move left out: by any amount for Search Better, and by
  0.50 pawns (the workbench's choice) for Search Clearly Better. UCI
  engines have no `minscore`, so the engine searches as for Next Best and the
  pane compares the scores.
* **Lock Engine** (Ctrl+Alt+L) keeps the kibitzer on its position while the
  board moves. The pane shows the move it is locked on. Releasing the lock
  follows the board again. **Goto Lock Position** (Ctrl+L) selects the locked
  position when its game is shown.
* **Paste Engine Variation** and **Paste Engine Move** are off while a
  threat is analysed, or while the engine is locked on another position. Their
  lines would not fit the current move.

Tests: an engine test sends `searchmoves` to the mock engine and refuses an
illegal one. App tests cover the null move, a refused threat in check, and
the moves left to search. Frontend tests cover Next Best, Search Better's
verdict, the threat toggle and the lock.

## Engine analysis history

Status: implemented on branch `engine-history` (2026-10-22). This is an
OpenFile feature; the reference app's "Display old moves (Next Best)" (3205)
only shows the moves Next Best left out.

* **History** in a kibitzer pane switches between the current lines and the
  engine's history for the position. The history keeps one row per choice:
  each time the best line's first move changes, a row starts. It holds the
  search time when the engine chose that move, and the latest depth, score
  and line while it stayed best. The latest choice comes first.
* The time is the engine's UCI `time`, now read from its `info` lines; an
  engine that sends none gets the time since the analysis started.
* A new position, a threat, Next Best or a restart starts a new history. At
  most 200 choices are kept, the oldest dropped first.

Tests: the protocol test reads `time`; a frontend test sends four lines with
two choices and checks the rows, their order and the reset on a new
position.

## Tactical Analysis (Blunder Check)

### What the reference app does

Tactical Analysis (`TACTICAL_ANALYSIS`, Ctrl+Alt+Shift+U, "Detect Blunders
and small tactics in the game") keeps its settings in
`TacticalAnalysisParams` (constructor `0x14042b3f0`, settings section "TA"):

| Setting | Key | Label (message) | Default |
|---|---|---|---|
| Time | `V_TIME` | Time [s] (93), "Recommended" (4191) | from an engine benchmark: 6,000,000 / speed, at most 20; 3 without one |
| Threshold | `V_THRESH` | Fine / Medium / Coarse (4166–4168) | first entry |
| Store evaluations | `V_EVALUATIONS` | 366 | off |
| Erase old annotations | `V_ERASE_VARIATIONS` | 327 | on |
| Training | `V_TRAINING` | 216 | off |
| Blunder categories | `V_MOVE_CATEGORIES` | 4768 | on |
| Replace (database only) | `V_REPLACE` | 69 | on |

When the dialog closes (`0x14042bf80`), the time becomes `Time × 500` ms,
and the threshold **Fine 35, Medium 55, Coarse 150** centipawns. The
threshold is raised by half when the time is under a third of the
recommended one. With no time set, a position gets 300 ms, or 800 ms or
1,200 ms on slower engines. "No blunders or combinations detected." (3860)
is its message for a clean game.

**What it writes.** This was traced in part, through an REA session
(`analysis/ta-symbols/`).
* **The job.** Tactical Analysis (`0x140a09920`) copies the settings into
  a job object (`0x1403f4fd0`, 0x208 bytes). Its run (`0x140414e20`) has
  phases: main-line analysis, evaluation symbols, verbal comments, threats,
  attack, initiative, pieces, and a game summary.
* **Evaluation symbols.** A main-line move gets an evaluation symbol when
  the symbol of the position after it differs from the one before it
  (`0x14041a460`, `0x14041dff0`). The move keeps its move symbol.

  The symbol comes from the score, from White's side (`0x140b560c0`):

  | Score (centipawns) | Symbol | NAG |
  |---|---|---|
  | above 160 | +− | 18 |
  | above 70 | ± | 16 |
  | above 30 | ⩲ | 14 |
  | −30 to 30 | = | 11 |
  | below −30 | ⩱ | 15 |
  | below −70 | ∓ | 17 |
  | below −160 | −+ | 19 |

  Mate scores get none. A later pass (`0x14041c580`) removes an evaluation
  symbol that repeats the previous one on the line.
* **Without a change of symbol** and without a verbal comment, the job sets
  the prefix symbol 142 ("better is") through `0x1403f9590`. Which node it
  lands on was not established.
* **Store evaluations** stores the reference app's own evaluation annotation
  (`0x1401a7b10`), not text.
* **Erase old annotations** also sets the annotator to "Tactical Analysis
  6.4 (Ns)" (format `"%s %s (%ds)"`, with the time in seconds).
* **Refuted alternatives** (`0x140413650`) get "?" when the score after
  them is below −150 and the loss is more than half the best score's size.
  They get "?!" when the score stays between −29 and 199 and the loss is
  81–149. The texts come from pools of messages 110070–110103 ("loses.",
  "Weaker is", "Don't play" and others).
* **Only moves** (`0x140408b80`, using the "!" literal) need the best
  line's lead to exceed the threshold and 30 % of the score.
* **Verbal comments** are drawn from the Analysis messages 110001–110695.
  One example is "was the only good move." The choice is partly random
  (`0x140ea40d0`).

How the reference app marks a played main-line move "?" or "??" was not found. No
"??" literal is used by the job's code, so the symbols are probably set as
numbers through a path not followed.

### Workbench

* The dialog has the reference app's **Time [s]** (default 3, 1 to 120; the engine
  gets Time × 500 ms for each position), **Threshold** Fine/Medium/Coarse
  (0.35, 0.55, 1.50 pawns) and **Store evaluations** (off). It adds a choice
  of sides (both by default) and of engine: an installed one, or a file.
* The check runs in the backend with its own engine process, separate from
  the kibitzers. Every position of the main line before or after a checked
  move is evaluated once, with progress, and **Stop** cancels it; a stopped
  check adds nothing.
* A move is a mistake when the engine's best line from the position before
  it scores at least the threshold more, for the mover, than the position
  after it, and the engine's first move is another one. Mates count as
  30,000 centipawns less the distance.
* Each mistake gets:
  * **?**, or **??** from three times the threshold, unless it already has
    a move symbol;
  * the engine's line, up to 16 moves, as a variation ending with its score
    and depth (`+0.30/20`);
  * with Store evaluations, the score after the move as text.
* **Evaluation symbols follow the reference app.** A checked move gets the symbol of
  the position after it when that differs from the symbol before it, using
  the table above. A symbol that repeats the previous one on the line is
  left out.
  * The new symbol replaces an evaluation the move already has (NAG 10, 11
    or 14–19).
  * It does not replace another position symbol, such as "with attack". A
    move with one is left as it is.
  * Unlike the reference app, existing repeated symbols are not removed.
* All of it is one edit, so Undo removes it. If the game changed while the
  check ran, nothing is added.
* **The workbench's own parts.** The ?/?? rule is the workbench's own, since
  the reference app's was not traced. So are the score as text, which stands in for
  the reference app's evaluation annotation, and the variation's score comment.
  Verbal comments, the "better is" prefix, Erase old annotations, Training,
  Blunder categories, the engine benchmark and the database mode are not
  ported.

## Deep Analysis and analysis jobs

See [analysis](analysis.md): the reference app's Deep Analysis schedule, as traced,
and the analysis jobs queue.
