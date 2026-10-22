# Evaluation profile and instant analysis

Status: implemented on branch `evaluation-profile` (2026-10-22). This spec was
written first. The branch is stacked on `engine-pane-commands`. The direction was
agreed in conversation.

Evidence is from the reference binary (x64), through an REA session, and
from the test database. Messages are cited by their ordinal in the message
catalogue.

## What the reference app does

### The annotation (type 0x26)

`EvalProfileAnno` (vtable `0x1411094b8`) holds an `EvaluationArr`
(`0x140c4d3a0`): 2,001 evaluations, one for each ply of the main line. Entry
0 is the start position and entry *i* the position after *i* plies. The
annotation sits at the game's start (position −1). It was found at no other
position in the test database.

An evaluation is stored as a packed 32-bit value (`0x140b55c10`, unpacked
by `0x140b55c30`):

| Bits | Field | Values |
|---|---|---|
| 31–24 | kind, signed | 0 centipawns, 1 mate, −1 none |
| 23–16 | pass or depth | see below |
| 15–0 | value, signed | centipawns, or moves to mate; from White's side |

* An empty entry is `0xFF000000` (kind −1, everything else 0). Profiles are
  written up to their last non-empty entry, so the count is that entry's index
  plus one.
* Kind 1's value counts the moves to mate. It is positive when White mates.
  0 is a mated position.
* Instant analysis writes its pass number (1, 2, …) into bits 23–16
  (`0x1403dd6d0`). The values 17–127 found in the test database look like
  engine depths from other tools; this was not traced.
* Kind 32 was found 5 times, with ordinary-looking values; its meaning was
  not traced.

**Classic CBA** (`0x14018f770` writes, `0x14018dae0` reads): the usual
6-byte header (BE24 position, type 0x26, BE16 length), then a BE16 count
*n* (at most 2,001), then *n* BE32 evaluations.

**2CBA** (`0x140190bf0` writes, `0x140190730` reads): a presence byte. When
it is non-zero, a sub-block follows (`0x140f04eb0`): an LE32 length, then
an LE16 count *n* and *n* LE32 evaluations (`0x140c4e9b0`, `0x140c4ec00`).

### Instant analysis

`INSTANT_ANALYSIS` (kc 181, `InstantAnalysisCmd` run `0x1409c6eb0`, "Instant
Analysis" 4379/4380) switches it on and off. `InstantAnalysisQueue`
(`0x1403dd310`) runs it on a worker thread; each game is a job (`0x1403dd760`):
* The engine evaluates each position of the main line in turn, for a time
  per position (`0x1403dd6d0`), and stores the result in the queue's profile
  at that ply.
* About every 250 ms a copy of the profile goes to the window, so the curve
  grows while the analysis runs.
* After a pass, with "Endless" set, the time doubles and another pass starts.
  The passes stop when the time per position exceeds 89,999 ms, when the job
  had its own time, or when the analysis is stopped.

Its settings (`0x1406c9350`, applied by `0x1406ca0c0`):

| Key | Label | Use |
|---|---|---|
| `V_GINSTANT` | | the group |
| `V_INSTANT` | "Enable" (470) | on/off |
| `V_IDEPTH` | | 1 to 6; the time per position is `V_IDEPTH` × 35 ms |
| `V_ENDLESS` | "Endless" (51347) | repeat with doubled time |

The default values were not traced.

### Showing the profile

* **Evaluation Profile** (`TOGGLEEVPROFILE`, kc 231, Ctrl+Alt+P,
  `ToggleEvProfileCmd` `0x140a0ed50`) shows or hides the profile pane
  (`EvalProfileWindow`, `EvaluationProfile`).
* **Show Evaluation Profile** (`NOTAEVALPROFILE`, kc 178,
  `NotaShowEvalProfileCmd` `0x1409c7f90` → `0x1409e5610`) shows the profile
  in the notation window.
* Delete Commentary has its own check box for profiles
  (`V_B_EVAL_PROFILE`).
* "Generate Eval Profiles" (51461) and "Find games with Similar Eval Profile"
  (51462/51463) are database commands. They were not traced.

**Not traced:** how the panes draw the curve (scale, colours, clicks), the
default settings, whether instant analysis writes its profile into the game,
and what happens to a profile when the main line changes.

## Workbench

### Reading and writing

* Both readers decode type 0x26 at the game's start into the game's profile:
  the packed values, kept as they are. A profile at another position is
  reported as unsupported, as before.
* The 2CBH writer writes the profile back as it was read: the presence byte
  and the sub-block. A game with a profile no longer counts as having
  unsupported annotations. Classic files are never written.
* PGN cannot hold a profile. Saving or exporting to PGN asks before dropping
  it, as for the other annotations PGN cannot hold.
* Validation: the classic and 2CBH test databases give the same profiles for
  their 24,783 shared entries.

### The profile pane

* **Evaluation Profile** (Ctrl+Alt+P) shows a pane under the board with the
  curve of the shown game. **Show Evaluation Profile** shows a slimmer curve
  above the notation. Both are switches, kept in the app's settings.
* The curve shows the main line's scores from White's side. The workbench
  chooses the scale: ±5 pawns, with larger scores and mates drawn at the edge.
  Plies without an evaluation leave a gap. The current move is marked.
* **Clicking** the curve selects the main-line move at that ply.
* **Critical moves** are marked on the curve. A move is critical when it
  loses at least 1.00 pawn for the side that played it (scores clamped to ±10
  pawns, mates at the clamp), unless that side is still more than 5 pawns up
  after it. This is the workbench's rule; the 1.00 pawn is the size of
  Centipawn Analysis's "Mistake". **Previous / Next critical move** step
  between them.
* The source shown is the instant analysis when it ran for this game and
  line, otherwise the profile stored in the game.

### Instant analysis

* **Instant Analysis** (Analysis ribbon) switches it on and off. Options keep
  its time (the reference app's 1 to 6, × 35 ms per position; default 3, the
  workbench's choice) and Endless.
* **Which engine:** the default engine from Engine Management, in a process
  of its own, separate from the kibitzers. Without a default engine, the
  command says so.
* **When:** each time a game tab shows another game, or its main line changes,
  the running analysis is cancelled and a new one starts. At most one runs.
* **What:** each position of the main line, at most 2,000 plies, as in the
  reference app. Progress and the growing curve come back about every 250 ms.
  Endless doubles the time after each pass, until it passes 90 s per
  position, as in the reference app.
* **Reuse:** evaluations are cached for the session by engine and position,
  with the time they had. Centipawn Analysis (and so the game review) and
  instant analysis add to the cache. A position already evaluated for at least
  the time asked is not sent to the engine again. At most 50,000 positions are
  kept.
* **Stop** in the pane cancels it. Switching instant analysis off cancels it
  too.
* The result is shown, not written into the game. Whether the reference app
  writes it was not traced.

## Tests

* Reader unit tests for both layouts, including the count limit and a short
  record.
* A round trip through the 2CBH writer keeps the profile byte for byte.
* App tests with the mock engine: instant analysis fills a profile for every
  ply, reuses cached positions, and stops when cancelled.
* Frontend tests: the pane draws the curve, a click selects the move, and the
  critical-move buttons step through the marked moves.
