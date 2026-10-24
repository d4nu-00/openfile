# Mate Training Builder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** From the games in the current list (for example a search for games
that end in mate), build training databases of mates in 1 to 5, balanced across
the *combinations of pieces involved in the mate*, each puzzle verified as a
forced mate in exactly N with a single mating first move:

* `<name> - Mate in 1.2cbh` … `<name> - Mate in 5.2cbh`, one per depth;
* `<name> - Mate chains.2cbh`: for each source game, the same mate as mate in
  1, then mate in 2, … up to how far it stayed a verified mate (backward chaining).

Every puzzle is stored with training questions, so the existing **Replay
Training** (Replay Selected Games) quizzes the user and tracks progress.

**Architecture:**
* An exact mate solver for 1 and 2 moves in `chess-core` (no engine needed).
* A `MateVerifier` trait with two implementations: the solver (depth 1–2) and a
  UCI engine (depth 3–5, e.g. Stockfish) driven through the existing
  `engine::EngineSession`.
* A pure builder in `src-tauri/src/mate_training.rs` that walks the source
  games, verifies chains step by step, fills quotas per (combination, depth),
  and produces `chess_core::Game`s with training questions; it is tested with a
  scripted fake verifier.
* A Tauri command that runs it with progress, Stop (keeps what was found) and
  Cancel, and writes the databases with `cbh_reader::v2::write::Writer`.
* A dialog in the database window, and an example binary for command-line use.

**Tech Stack:** Rust (shakmaty 0.30, chess-core, cbh-reader, engine), Tauri,
React + TypeScript, Vitest.

**Builds on (already on `exp`):** `cbh_reader::ending::mating_pieces` (the
pieces involved in a mate; definition in docs/search.md, "Ending tab"),
`search::final_position`, the Ending tab's "mate by" search, Export PGN.

## Definitions (fixed — do not reinterpret)

* **Chain.** Let the source game's main line end in checkmate after ply `L`
  (the last move is the mating side's). The **step-i position** is the
  position before ply `L - 2(i - 1)` (i.e. `2i - 1` plies before the end), for
  `i = 1..=5`, provided it exists. The mating side is to move there. Step 1 is
  the position before the mating move. Chess960 games and games whose main line
  does not end in mate are skipped.
* **Valid step.** Step `i` is valid when, in the step-i position:
  1. the side to move has a forced mate in exactly `i` moves (none shorter);
  2. exactly one first move forces mate in `i` (**unique**);
  3. that move is the game's move.
  The chain of a game is steps `1..=k` where every step up to `k` is valid and
  step `k + 1` is not (or does not exist, or `k` reached the maximum depth
  asked). A chain stops at its first invalid step; later steps are never used.
  Because every step is valid, every mating-side move of a step-i puzzle is the
  unique fastest mate at its point, so each can be a training question.
* **Combination.** The pieces involved in the final mate
  (`cbh_reader::ending::mating_pieces` on the final position), written in the
  order Q, R, B, N, P, K with `+`, e.g. `Q+N`, `R+K`, `Q+B+P`, `N`. Bishops and
  knights are distinct by default; with **Bishop = Knight** both are written `m`
  (minor), e.g. `Q+m`. The king **is** part of the combination.
* **Quota.** `per` puzzles for each (combination, depth) in the depth
  databases, and `per` chains (of length ≥ 2) for each combination in the chain
  database. A combination filter, when given, is a list of combinations; only
  those are collected (compare after normalising order and spaces, e.g.
  `"n + q"` → `Q+N`).
* **Duplicates.** A puzzle is identified by its position (board, side to move,
  castling, en passant). A position already collected for that depth is
  skipped (the Mega database has many duplicate games).
* **Order inside each depth database:** round-robin over combinations in order
  of first appearance (the 1st puzzle of each combination, then the 2nd of
  each, …), so practice interleaves patterns. The chain database keeps each
  chain together, mate in 1 first, chains in the order found.

## Puzzle games

A step-i puzzle is a `chess_core::Game`:
* start position (`start_fen`) = the step-i position; main line = the source
  game's remaining `2i - 1` plies; no variations, no other annotations;
* a move-input training question on **each mating-side move** of the line
  (i questions), answer = that move (one answer, its points 1), question score 1,
  `seconds` 0 if 0 means "no limit" in the existing training code — otherwise
  the TrainingDialog default; mirror how `TrainingDialog.tsx` / the training
  writer encode promotions (mating by promotion must round-trip);
* question text (English slot): first question `"White to move: mate in 3"` /
  `"Black to move: …"`; later questions `"Continue: mate in 2"`, `"Mate in 1"`;
* `help[0]` (first hint) on the first question: `"Pieces in the mate: Q+N"`;
* wrong-answer text: `"Not the mate. Look at the squares around the king."`;
* tags: keep White, Black, Event, Site, Date, Round, Result, Elo of the source
  game; set Annotator to `OpenFile mate training`; add a text comment before the
  first move `"Mate in 3 · Q+N · from game 12345"` (the source entry number);
* chain database entries are the same games as the depth databases' (built the
  same way), one per step.

## Global Constraints

- Branch `exp` only; commit after each task; imperative subject; every commit
  message ends with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
  No push, no PR. Leave untracked `.claude/` and `exp-output/` alone.
- Source databases are read-only. Output databases are new files; refuse to
  overwrite existing ones (`AppError` "exists"), and never write beside a source.
- Never commit games from the user's databases.
- Match surrounding style (comment density, naming, `AppError` codes,
  `blocking(...)`, progress `Channel`s, decimal strings in the frontend API,
  cancel maps like `paste_cancels` / `export_cancels`).
- New docs files must be added to the allow-lists in
  `scripts/check-distribution.mjs` (and `tests/distribution.test.ts` if it
  lists them). This plan file is already on the list.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
  `cargo fmt --all --check`, `npm run typecheck`, `npm test`,
  `npm run check:distribution` must pass at the end of every task.

## Review Focus

- Step indexing (`2i - 1` plies before the end), the mating side to move, and
  stopping a chain at the first invalid step.
- "Exactly i" and "unique" in both verifiers; the engine verifier must not
  call a position valid on a timeout without a mate score.
- Quotas, dedupe and round-robin order; a full quota for depth `i` must not stop
  the chain from being verified further when a higher depth still has room.
- Stop writes what was found; Cancel writes nothing; outputs never overwrite.

---

### Task 1: Exact mate solver for 1 and 2 moves (chess-core)

**Files:** new `crates/chess-core/src/mate.rs`, `crates/chess-core/src/lib.rs`

- [ ] `pub fn mates_in_one(pos: &shakmaty::Chess) -> Vec<shakmaty::Move>` — legal
  moves after which the opponent is checkmated.
- [ ] `pub fn mates_in_two(pos: &Chess) -> Vec<Move>` — moves `m` such that
  `pos` has no mate in one, and after `m` the opponent is not stalemated, has at
  least one legal move, and every reply allows a mate in one (a reply that is
  itself checkmate cannot happen; stalemate after `m` fails).
- [ ] `pub fn shortest_mates(pos: &Chess, max: u8) -> Option<(u8, Vec<Move>)>`
  for `max <= 2`: the smallest `n <= max` with mates and those moves.
- [ ] Failing tests first: back-rank mate in 1 (unique); a position with two
  mates in 1 (not unique); a known mate in 2 (e.g. a queen sacrifice into a
  smothered or back-rank mate — construct and check with python-chess
  `python3 -I -c` if needed) with exactly one key move; a mate-in-2 position
  that also has a mate in 1 (shortest = 1); a stalemate trap where the "key"
  stalemates.
- [ ] Commit.

### Task 2: Verifiers

**Files:** new `src-tauri/src/mate_training.rs` (module may be split into
`mate_training/{mod,verify,build}.rs` — your call), `src-tauri/src/lib.rs`

- [ ] ```rust
  pub enum StepCheck { Valid, NotMate, Shorter(u8), Longer, NotUnique, OtherMove, Timeout }
  pub trait MateVerifier {
      /// Checks the step-`n` position `pos`, whose game move is `played`.
      fn check(&mut self, pos: &Chess, n: u8, played: &Move) -> Result<StepCheck, AppError>;
  }
  ```
- [ ] `SolverVerifier` for `n <= 2` using Task 1.
- [ ] `EngineVerifier` (spawned from an `EngineSpec`, `MultiPV` = 2 set once,
  `Threads`/`Hash` left at the engine defaults): for `n >= 3`, `analyse` the
  position (as `blunder_check::evaluate_to` does), keep the latest info per
  multipv line, stop when line 1 has `Score::Mate { moves: n }` (positive) at
  depth ≥ `2n + 4`, or at the time cap (`millis`, default 1000). Then:
  line 1 not a positive mate → `NotMate` (or `Timeout` if no score arrived);
  mate in fewer → `Shorter`; more → `Longer`; line 2 a positive mate in ≤ n →
  `NotUnique`; line 1's first move ≠ `played` → `OtherMove`; else `Valid`. A
  position with only one legal move has no line 2 — that is unique.
- [ ] A `CombinedVerifier` that uses the solver for n ≤ 2 and the engine above.
- [ ] Tests: solver verifier on Task 1 positions; engine verifier with the
  existing `mock-uci` test engine only where its scripted output makes sense
  (e.g. a timeout/no-mate case); a real-engine test gated on the env var
  `OPENFILE_TEST_STOCKFISH` (path), `#[ignore]` otherwise, checking a known mate
  in 3 is `Valid`, and the same position with the game move replaced is
  `OtherMove`.
- [ ] Commit.

### Task 3: Builder (pure)

**Files:** `src-tauri/src/mate_training.rs`, new `src-tauri/tests/mate_training.rs`

- [ ] `pub struct BuildOptions { min_depth: u8, max_depth: u8 /*1..=5*/, per: u32, combinations: Option<Vec<String>>, pool_minors: bool }`
  (serde camelCase for the frontend).
- [ ] `pub fn combination(t: &Tally, pool_minors: bool) -> String` and
  `pub fn normalise_combination(s: &str, pool_minors: bool) -> Option<String>`.
- [ ] `pub struct Collected` holding the per-depth puzzles (with combination and
  source ordinal), the chains, counts of rejections by `StepCheck` kind, games
  scanned, and a `full()` check.
- [ ] `pub fn add_game(&mut Collected, ordinal, game: &chess_core::Game, opts, verifier) -> Result<(), AppError>`:
  final position must be mate (else count `notMateGame`); compute the
  combination (skip if filtered out); verify steps `1..=max_depth` in order,
  stopping at the first invalid step **or** once no depth ≥ the next step and
  no chain slot for this combination has room; record each valid step `i ≥
  min_depth` whose (combination, i) quota has room and whose position is not a
  duplicate; record the chain when its length ≥ 2 and the chain quota has room.
- [ ] `pub fn puzzle_game(source: &Game, step: u8, combination: &str, ordinal: u64) -> Result<Game, AppError>`
  per "Puzzle games" above, and `pub fn ordered(&Collected) -> (Vec<Vec<Game>> /*per depth*/, Vec<Game> /*chains*/)`
  applying the round-robin order.
- [ ] Failing tests first, with a fake verifier scripted per (position FEN, n):
  chain stops at first invalid step; quotas respected; dedupe; a full depth-1
  quota still lets depth 2 fill; combination filter and normalisation;
  round-robin order; puzzle game has the right start FEN, plies, `i` training
  questions with the right answers, hint text and comment; a promotion mate
  round-trips through `Writer` + reading back (use a temp dir, as other tests do).
- [ ] Commit.

### Task 4: Command, writing, example

**Files:** `src-tauri/src/mate_training.rs`, `src-tauri/src/commands.rs`,
`src-tauri/src/lib.rs`, `src-tauri/src/registry.rs` (cancel map), new
`src-tauri/examples/build_mate_training.rs`

- [ ] Tauri command `build_mate_training(database_id, ranges: Vec<RowRange>, options: BuildOptions, engine_id: String, millis: u64, folder: PathBuf, name: String, events: Channel<MateTrainingProgress>) -> MateTrainingResponse`.
  Ranges are the rows to use (the frontend sends the whole list, or the
  selection). Read games with a separate `cbh_reader::Database` handle as
  `export.rs` does. Resolve `engine_id` from the app settings the way Blunder
  Check resolves its engine. Progress (≤ 4 per second): games scanned / total,
  puzzles per depth, chains, current combination counts. `stop_mate_training`
  (write what was found) and `cancel_mate_training` (write nothing).
- [ ] Output: `<folder>/<name> - Mate in <n>.2cbh` for each depth in range that
  has at least one puzzle, and `<folder>/<name> - Mate chains.2cbh`; refuse if
  any target exists; `save::reject_source_destination` against the source.
  Response: paths written, counts per (combination, depth), chain counts,
  rejections by kind, games scanned, stopped/cancelled.
- [ ] Example `build_mate_training --path <db> --filter '<GameFilter JSON>' --engine <exe> --millis 800 --min 1 --max 5 --per 20 [--combinations "Q+N,R+K"] [--pool-minors] --out <folder> --name <name>`:
  runs `db.search`, then the same builder and writer, printing progress lines
  and the final counts table. (Acceptance uses it.)
- [ ] Tests: a temp source database built from PGN with a few mates (as earlier
  tests do), the fake verifier for depth ≥ 3, checking the written databases
  read back with the expected game counts, order and training questions;
  existing-target refusal; Stop vs Cancel.
- [ ] Commit.

### Task 5: Dialog (frontend)

**Files:** `src/api/client.ts`, `src/api/types.ts`, `src/commands/catalogue.ts`,
`src/App.tsx`, new `src/components/MateTrainingDialog.tsx`, `src/styles/shell.css`,
`src/dev/demoApi.ts`, `src/tests/harness.tsx`, new `src/tests/mateTraining.test.tsx`

- [ ] Command `build-mate-training`, label **"Build Mate Training"**, tip "Make
  mate-in-1 to mate-in-5 training databases from the games in the list", an
  icon (e.g. lucide `Crosshair` or `Target`), `views: db`, in the database
  ribbon next to the Replay Selected Games / training commands. Enabled when a
  database is open, `w.rowCount > 0`, and no build is running.
- [ ] Dialog fields (accessible names as quoted): **"Source"** radio: "All
  games in the list (N)" / "Selected games (M)"; **"From depth"** and **"To
  depth"** selects 1–5 (default 1 and 5); **"Puzzles per combination"** number
  (default 20, 1–1000); **"Combinations"** radio "Every combination found" /
  "Only these" with a text field **"Combination list"** (placeholder
  `Q+N, R+K, B+N`) validated with the same normalisation rules (mirror them in
  TS; show an inline error and disable Build for an unknown piece letter);
  **"Bishop = Knight"** checkbox (default off); **"Engine"** select from the
  configured engines (default engine preselected; if none, a note to add one
  with Add Kibitzer and Build disabled); **"Time per check (ms)"** (default
  1000); **"Folder"** with a Choose… button (folder picker) and **"Name"** text
  (default `Mate training`). A hint line: "Mates in 1 and 2 are solved exactly;
  3 to 5 are checked with the engine. Only positions with a single mating move
  are kept."
- [ ] While running: progress (games scanned / total, puzzles per depth,
  chains), **Stop** (keeps what was found) and **Cancel**. At the end: a summary
  table combination × depth, the chain count, rejections, and the paths, with
  an **Open** button per database (open it as a database tab, like Open Database).
- [ ] Demo API stub so `npm run dev` (`/?demo`) shows a plausible run.
- [ ] Failing tests first: the command opens the dialog; Build sends the
  ranges, options, engine id, millis, folder and name; an invalid combination
  disables Build; Stop calls the stop API; the summary renders counts.
- [ ] Commit.

### Task 6: Docs

**Files:** new `docs/mate-training.md`, `docs/training.md` (link),
`feature_checklist.md`, `README.md` (one bullet clause), distribution allow-lists

- [ ] `docs/mate-training.md`: what it builds; the Definitions (chain, valid
  step, combination, quota, duplicates, order); exact solver vs engine and the
  engine's limits (a uniqueness check can only see what the engine finds in its
  time; raising the time makes it stricter); suggested workflow: search
  (Game data › Final position › Mate, perhaps Year/Elo; or Ending › Mate by),
  Build Mate Training, open a "Mate in N" database, Select All, Replay
  Selected Games; for backward chaining use the chains database in order.
- [ ] Add the new doc to `scripts/check-distribution.mjs`.
- [ ] Commit.

### Task 7: Verification (report output; do not fake it)

- [ ] All commands from Global Constraints — paste the summary lines.
- [ ] Acceptance on the real read-only database, release build, outputs only in
  the scratchpad directory given in the prompt:
  `build_mate_training --path "<Mega>" --filter '{"mate":true,"yearMin":2018,"eloMin":2200,"eloMode":"average"}' --engine /opt/homebrew/bin/stockfish --millis 800 --min 1 --max 5 --per 10 --out <scratch> --name "Mega mates"`.
  If that takes more than ~40 minutes, stop it and rerun with `--per 5`.
  Report the counts table, chains, rejections, time, and paths.
