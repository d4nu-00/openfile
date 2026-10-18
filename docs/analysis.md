# Deep Analysis and analysis jobs

Status: implemented on branch `deep-analysis` (2026-10-15); this spec was
written first. The direction was agreed in conversation; the user asked to
proceed without review gates.

Evidence is from the reference binary (x64), analysed statically through an REA
session (`analysis/deep-analysis/` in the research workspace). Messages are
cited by their ordinal in the message catalogue.

## What the reference app does

### Commands

| Command | Id | Run slot | Message |
|---|---|---|---|
| Deep Analysis | `DEEP_ANALYSIS` (`DeepAnalysisCmd`) | `0x140a000d0` | "Deep Analysis" (3648) |
| New analysis jobs | `NEW_ANALYSIS_JOBS` | `0x140a00390` | "Create a new collection of analysis jobs" (4023) |
| Open analysis jobs | `OPEN_ANALYSIS_JOBS` | `0x140a00460` | "Open an existing collection of analysis jobs" (4025) |
| Add Analysis Job | `ADD_ANALYSIS_JOB` | `0x1409ffec0` | "Add another analysis job to the queue of waiting jobs" (4009) |
| Create analysis jobs | `CREATE_CORR_ANALYSIS_JOBS` | `0x1408b6750` | "Create analysis jobs from the final positions of the selected games" (51281) |

None has a default key. Each run slot casts the active frame to the game
window and calls its action: Deep Analysis `0x140a092a0`, the two job
collections `0x140a0a1a0` (with a flag for "new"), Add `0x140a085f0`.

### Deep Analysis

* **Entry.** With fewer than two engines loaded, `0x140a092a0` runs the
  analysis on the game window's kernel (`0x140a02180`), under a modal
  progress loop titled "Deep Analysis". With two or more it runs the same job
  with several engines (`0x140a08d70`, "Cloud Analysis" 3848, warning "You
  should use at least two engines." 3868).
* **The job.** `0x140a02180` builds a 0x1598-byte job (constructor
  `0x140422d70`) and runs it (`0x140427060`). Its settings object is
  `CloudAnalysisVariables` (settings section `CloudAnalysis`). For every
  engine (at most 9) it has `GROUP<n>` (the engine, labelled "Local Engine"
  3851 or "Cloud Engine" 3852), `TypeChoice<n>` (its role, three choices
  built by `0x140428540`) and `NVARS<n>` ("Variations" 497, 1 to 10, default
  3, or 1 for role 3); see `0x140424c20`. The roles' labels are probably
  "Root Candidates" (3854), "Expected Answers" (3855) and "Analysis Crawler"
  (3853), by their position in the catalogue; this was not confirmed.
* **Time.** The job takes a time limit in seconds (`param_4 × 1000`, offset
  0x2ec). From the menu it is 0, so the analysis runs until stopped. As an
  analysis job it is the job's time, at least 30 s (`0x140a030c0`).
* **Iterations.** The run (`0x140427060`) repeats iterations 0, 1, 2, … and
  logs "Deep Analysis: Iteration = n, Time per move = t s" (3648, 3850, 637).
  Iteration *i* sets (`0x14042a790`):
  * the base time per position *t* = round(4 × 1.7^*i*) seconds, or
    round(3.9 × 1.6^*i*) when candidate engines run beside it;
  * the depth *D* = ⌊√(0.6 *i*) + 2.5⌋ plies;
  * the widths ⌊√(0.3 *i*) + 3.5⌋ at the root and ⌊√(0.3 *i*) + 1.5⌋ at the
    reply, 1 below (`0x14042a5f0`; with one side analysed the widths go to
    that side's plies);
  * a score window of 70 centipawns (offset 0xe4).

  | Iteration | *t* | Depth | Root width | Reply width |
  |---|---|---|---|---|
  | 0 | 4 s | 2 | 3 | 1 |
  | 1 | 7 s | 3 | 4 | 2 |
  | 2 | 12 s | 3 | 4 | 2 |
  | 4 | 33 s | 4 | 4 | 2 |
  | 7 | 164 s | 4 | 4 | 2 |
  | 14 | 6,735 s | 5 | 5 | 3 |
* **Time per position** (`0x1404280e0`): *t* + *t*/2 at the root, 0.7 *t*
  at the deepest ply, *t* elsewhere, at least 3 s.
* **The search** (`0x140425590`, started with the window −30000 to 30000 at
  ply 0) is recursive and builds the tree in the game itself:
  1. At a node of ply *p* the engine is asked for *k* lines (`0x140427780`):
     the ply's width, less one when *p* > 0, the width is over 1, the node
     has more than two siblings and its index is below half of them; and at
     least one more than the node's children already in the tree.
  2. The lines are cut where a line scores more than 70 below the best or
     more than 70 below the line before it (`0x140b94da0` with 70, 70); at
     most 10 are kept.
  3. Their first moves are played into the game as variations (existing
     moves are followed), and the search recurses into each, until depth
     *D*.
  4. At the depth limit, a move gets the reference app's evaluation symbol for its
     score (`0x140425370` through `0x140b560c0`, the same rule as Tactical
     Analysis; see [engines](engines.md)) and the reference app's own evaluation
     annotation (`0x1401a7b10`, not decoded).
  5. New lines are coloured while the analysis runs (`0x14042a960`). The
     colours are removed at the end (`0x140429c30`). Move symbol 8 is set or
     cleared from a flag the engine step returns (`0x14042a8f0`); its meaning
     was not traced.
* **Stopping.** The run stops when the user stops it. With a time limit it
  also stops after an iteration once 80 % of the limit has passed, or when
  the next iteration is expected to take more than 125 % of it. Within an
  iteration it stops searching beyond 150 % (`0x140425590`).
* **Saving.** Every 300 s, and after each iteration, a copy of the game is
  saved through `0x140426450`, apparently to a database of the analysis.

### Analysis jobs

* **A job** (`0x1403d08f0`, a 0x580-byte record) holds a copy of the game up
  to the current position, the path of its database and its game number
  when it has one, and its settings:
  * type: 1 Analysis or 2 Deep Analysis;
  * engine (offset 0x558);
  * time (0x550; at least 15 s for Analysis and 30 s for Deep Analysis);
  * lines for Analysis ("Variations", 0x548);
  * a depth to stop at instead of the time (0x54c, with a flag in 0x544);
  * flags (bit 0: "Merge result with original game", 4072).
  A job without settings of its own uses the collection's.
* **A collection** is a file in the reference app's `AnalysisJobs` folder, read with
  a 1 MB limit (`0x1403d0fa0`). "Add Analysis Job" appends the current game
  and says "Analysis job added" (4026).
* **The window** is `AnalysisSetupVariables` ("Analysis Setup", 4010;
  `0x14004f8b0`), with settings keys `V_JOB_LIST`, `V_JOB_ACTIONS`,
  `V_JOB_SETTINGS`, `V_DEFAULT`, `V_TIME` and `V_RESULTS`. The job list's
  menu has Restart (4065), Skip (4066), Default (587), Remove (4068) and
  Properties (361). The window also has Start/Continue (4063), Restart (4064),
  Add Position (4067), "Total time" and "Remaining time" (4069, 4070,
  `0x1403d19f0`) and Show results (4071).
* **Running** (`0x140a0a510`): "You have to select an engine for analysis"
  (4011) and "There are no open positions" (4014) refuse a start. Otherwise
  the open jobs run one after another until stopped. Each is marked running,
  then done or failed.
  * Analysis (`0x140a02d10`) runs the engine with the job's lines until the
    time (at least 15 s) or the depth is reached. It stores the lines every
    10 s (`0x140a019b0`).
  * Deep Analysis (`0x140a030c0`) runs the job above with the job's time.
  * After each job its game is saved (`0x140a032f0`). With "Merge result with
    original game" (`0x140a02460`) the original game is loaded from its
    database by number and compared with the job's game. If they match, the
    job's result is merged into it and it is replaced.

**Not traced.** The meaning of the three roles and of move symbol 8, the
multi-engine split, the jobs file format, the analysis database the job
saves to, and the merge itself (`0x140b64220`).

## Workbench

### Deep Analysis

**Deep Analysis** (Analysis ribbon) analyses the board position of the shown
game as the reference app does, with one engine: an installed one or a file.

* **The dialog** offers the engine and the duration: until stopped (as from
  the reference app's menu) or a time limit in minutes (1 to 1,440).
* **The job** runs in the backend with its own engine process, separate from
  the kibitzers. It follows the reference app's iterations, time per position, depth,
  widths, sibling rule and 70-centipawn window above. The reference app's
  multi-engine roles are not ported. The reference app's time-limit rules apply.
* **The tree** is the job's own while it runs. A node stores its move, and,
  from the line that found it, the score for the side that played it and the
  engine depth. Moves the game already has at the analysed position are
  followed.
* **Progress** shows the iteration, the positions analysed, the elapsed time
  and the best line found so far, with its score. The best line is the
  backed-up (negamax) value of the tree, and children are ordered by it.
  This is the workbench's own choice: the reference app's minimax window was seen
  but its use not traced.
* **Stop** ends the analysis and keeps what was found, as in the reference app. The
  dialog's Cancel discards it.
* **The result is one edit**, so Undo removes it, and nothing is added if
  the game changed meanwhile:
  * every root-to-leaf path becomes a line from the analysed position, best
    first;
  * the last move of each line gets the reference app's evaluation symbol for its
    score;
  * it also gets the score and depth as text (`+0.35/20`). This stands in
    for the reference app's evaluation annotation, which is not decoded.

  Line colours are not used, since the tree is only written at the end. The
  move symbol 8 rule is not ported.
* **Bounds:**
  * at most 400 nodes in the tree, after which it stops growing and says so;
  * at most 30 iterations;
  * a depth of at most 12 plies;
  * at most 10 lines per position.

### Analysis jobs

The jobs queue analyses several games of a 2CBH database opened for editing
and replaces each game with the analysed one. That is the reference app's "Merge
result with original game", always on.

* **Adding jobs.** In the game list of an editable 2CBH database:
  * **Create analysis jobs** adds the final position of each selected game's
    main line, as the reference app's command does;
  * **Add Analysis Job** in the game window adds the board position of the
    shown game, if it was loaded from such a database and has no unsaved
    changes.

  A job stores the database, the entry number, the moves leading to the
  position and a fingerprint of the game. The commands sit in the database
  window's Home tab and the game window's Analysis tab. The reference app's ribbon
  places for them were not traced.
* **The queue** is kept in the backend for the session, at most 1,000 jobs.
  the reference app's jobs file is not written or read. **New analysis jobs** clears
  the queue, and **Open analysis jobs** shows it.
* **The window** lists the jobs (number, game, move, type, status) and has:
  * Start/Continue and Stop;
  * Restart, Skip and Remove for the selected job;
  * the settings for new jobs: type Analysis (time, lines) or Deep Analysis
    (time), and the engine.

  The time is at least 15 s for Analysis and 30 s for Deep Analysis, as in
  the reference app.
* **Running.** One engine process runs the waiting jobs in order, with
  progress for the queue and the current job. For each job:
  1. The game is loaded from its database and the position found again by
     its moves.
  2. The job is skipped, with the reason, when:
     * the position is gone;
     * the game is open in a tab;
     * its annotations were not fully decoded, since replacing would lose
       them;
     * it is a Chess960 game.
  3. **Analysis** runs the engine for the time with the job's lines, and
     adds them as variations ending with their score and depth. **Deep
     Analysis** runs the analysis above for the time.
  4. Before writing, the entry is read again. If it changed since step 1,
     the job fails and nothing is written. Otherwise the game is replaced
     and the opening tree's journal records the change.
* **Stop** halts the running job, which goes back to waiting with nothing
  written.

## Tests

* Pure tests for the iteration schedule, the time per position, the line
  cut and the sibling rule.
* App tests with `mock-uci legal`, a mock mode that gives scored MultiPV
  lines of legal moves for any position:
  * Deep Analysis builds a bounded tree and adds it as one undoable edit;
  * Stop keeps the tree;
  * a changed game adds nothing;
  * the jobs queue replaces the games of a scratch 2CBH and skips an open
    game;
  * Stop leaves the jobs waiting.
* Frontend tests for both dialogs.
