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
and the threshold **Fine 35, Medium 55, Coarse 150** centipawns, raised by
half when the time is under a third of the recommended one. How the search
then uses them, and which symbols and texts ChessBase writes, were not
traced; "No blunders or combinations detected." (3860) is its message for a
clean game.

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
* Each mistake gets **?**, or **??** from three times the threshold, unless
  it already has a move symbol; the engine's line (up to 16 moves) as a
  variation ending with its score and depth (`+0.30/20`); and with Store
  evaluations, the score after the move as text. All of it is one edit, so
  Undo removes it. If the game changed while the check ran, nothing is
  added.
* The symbol rule is the workbench's own. Erase old annotations, Training,
  Blunder categories, the engine benchmark and the database mode are not
  ported.
