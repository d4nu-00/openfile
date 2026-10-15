# Engines: kibitzers, lines, pasting and Engine Management

Status: implemented on branch `game-tabs` (2026-10-13). The direction was
agreed in conversation; the user asked to proceed without review gates.

## What ChessBase does

Evidence is from CBase18.exe 18.1 (x64), its command table and messages.

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
  Alt+F2, which is ChessBase's key for Default Kibitzer.

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
  form, chosen because ChessBase's own evaluation annotation is not decoded;
  it is plain text in 2CBH and PGN.

### Engine Management

The installed engines are kept in the app's settings file
(`settings.json` in its config folder) with a name, the executable's path
and the options changed for it. **Add engine…** asks for an executable and
names it from its UCI handshake (`id name`); the first engine becomes the
default. Options changed in a kibitzer of an installed engine are kept and
applied the next time it starts. At most 32 engines are kept; paths must be
absolute.

## Chess960

A position whose castling rooks stand off their standard squares is a
Chess960 position. Before analysing it, a session sets `UCI_Chess960` to
true, and sets it back to false for the next standard position. Castling
moves go to the engine king to rook. An engine without the option is refused
with a message. Chess960 games whose castling matches the standard squares
are sent as standard chess, which plays the same. Users cannot set
`UCI_Chess960` themselves.

## Syzygy tablebases

ChessBase has "Use Tablebases" (message 737) and a "Tablebases" page (2554);
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

### What ChessBase does with tablebases

Evidence: CBase18.exe 18.1.
* Its Options keep Syzygy and Nalimov folders (`TablebaseOptions`,
  settings key `V_TABLEBASES`; strings "Syzygy (GUI)" and "Syzygy (Engine)").
* It hands the Syzygy folder to engines with `setoption name SyzygyPath
  value …`, and a Nalimov path the same way.
* "Use Tablebases" (737) is used by two functions (`0x140066670`,
  `0x140075690`) that were not followed.
* The search mask's Material tab (dialog 328) has a "Tablebase" checkbox
  (1090).

No ChessBase command annotates a game from the tables, and how ChessBase
shows tablebase results in the notation was not traced.

### Engines and the notation (workbench)

* **SyzygyPath.** Each engine that offers `SyzygyPath` is pointed at the
  chosen folder when it starts. That covers kibitzers and Tactical
  Analysis, as in ChessBase. Engines without the option are left as they
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

## Tactical Analysis (Blunder Check)

### What ChessBase does

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
* **Store evaluations** stores ChessBase's own evaluation annotation
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

How ChessBase marks a played main-line move "?" or "??" was not found. No
"??" literal is used by the job's code, so the symbols are probably set as
numbers through a path not followed.

### Workbench

* The dialog has ChessBase's **Time [s]** (default 3, 1 to 120; the engine
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
* **Evaluation symbols follow ChessBase.** A checked move gets the symbol of
  the position after it when that differs from the symbol before it, using
  the table above. A symbol that repeats the previous one on the line is
  left out.
  * The new symbol replaces an evaluation the move already has (NAG 10, 11
    or 14–19).
  * It does not replace another position symbol, such as "with attack". A
    move with one is left as it is.
  * Unlike ChessBase, existing repeated symbols are not removed.
* All of it is one edit, so Undo removes it. If the game changed while the
  check ran, nothing is added.
* **The workbench's own parts.** The ?/?? rule is the workbench's own, since
  ChessBase's was not traced. So are the score as text, which stands in for
  ChessBase's evaluation annotation, and the variation's score comment.
  Verbal comments, the "better is" prefix, Erase old annotations, Training,
  Blunder categories, the engine benchmark and the database mode are not
  ported.

## Deep Analysis and analysis jobs

See [analysis](analysis.md): ChessBase's Deep Analysis schedule, as traced,
and the analysis jobs queue.
