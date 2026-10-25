# Spaced Repetition Decks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Chessable/Anki-style training. Turn a PGN file, or the games of a
database list, into a **deck** of lines; study it day by day, where the user
plays one side's moves on the board, mistakes bring a line back soon, and lines
that are known come back at growing intervals.

Typical uses: the mate-training databases/PGNs (`Mate in 3.pgn`, the chains),
a repertoire PGN with variations, a collection of puzzle positions (PGN games
with a `FEN` tag and the solution moves).

**Architecture:**
* `src-tauri/src/srs/` — pure Rust: deck model, line extraction from
  `chess_core::Game`s, the scheduler (Anki-style SM-2, below), the study queue;
  plus a store in the app data folder (`srs/decks/<id>.json`, atomic writes,
  like `replay_training.rs`) and Tauri commands.
* Frontend: a **Decks** dialog (list, create, options, delete) and a **Study**
  view (board, move input, feedback, grade buttons), reusing the board
  interaction of `ReplayTrainingSession.tsx` (click or drag, promotions,
  `legalUci`).

**Tech Stack:** Rust (chess-core, shakmaty, serde_json), Tauri, React +
TypeScript, Vitest.

## Definitions (fixed — do not reinterpret)

### Cards

* A **card** is one line: a start position (FEN) and a list of moves (UCI and
  SAN), plus the **trainee colour**. The user plays the trainee's moves; the
  opponent's moves are played automatically (after ~400 ms, so they can be seen).
  A line whose first move is the opponent's starts by playing it.
* **Lines of a game:** with *Include variations* on (default), every
  root-to-leaf path of the game tree is a line; off, only the main line. A
  path's trailing opponent moves (after the trainee's last move) are dropped; a
  path with no trainee move is not a card.
* **Trainee colour** (deck option): *Side to move* (default; the side to move in
  the game's start position — right for puzzles), *White* or *Black* (for
  repertoires). With White/Black, lines are kept as they are.
* **Card id:** SHA-256 (hex, first 16 bytes) of `start FEN (first 4 fields) +
  " " + moves UCI joined by spaces + " " + trainee colour`. Duplicate ids within
  a deck keep the first.
* Each move keeps the PGN comment after it (shown once played) and the card
  keeps the source game's White, Black, Event, Date and a source label (file
  name or database name + entry number). Annotations other than text comments
  are not used.
* **Accepted moves:** at a trainee move, the card's move is correct. If the user
  plays another move that some *other* card of the deck plays from the same
  position (same FEN, first 4 fields) at that point, show "That move belongs to
  another line in this deck — play this line's move" and let them try again,
  **without** counting a mistake. Any other move is a **mistake**: show
  "Not this line's move", reveal the correct move as an arrow, and require it
  to be played before continuing.
* Limits: at most 20,000 cards per deck and 400 plies per line; a source with
  more is truncated, and the result says how many were left out.

### Scheduling (Anki SM-2 variant)

Card state: `kind` ∈ {`new`, `learning`, `review`, `relearning`}, `step` (index
into the learning or relearning steps), `due` (ms since epoch), `interval`
(days, float), `ease` (starts 2.5, never below 1.3), `reps`, `lapses`.
Deck options with defaults: learning steps `[1, 10]` minutes, relearning steps
`[10]` minutes, graduating interval 1 day, easy interval 4 days, new cards per
day 20, reviews per day 200, maximum interval 36500 days, new card order
*source order* (default — keeps the mate chains in their order) or *random*.

Grades: **Again**, **Hard**, **Good**, **Easy**.
* **Automatic grading:** a card played with a mistake is graded **Again**
  (after the user has completed the line). A card completed with a hint used
  and no mistake may be graded Hard at most (Good and Easy disabled). A clean
  card shows Hard / Good / Easy, Good default (Enter). Keys 1–4 for the grades.
* `new` / `learning` / `relearning` (with steps `S`, current `step`):
  Again → `step = 0`, `due = now + S[0]`; Hard → `due = now + S[step]` (same
  step; for the last or only step use `1.5 × S[step]`); Good → `step + 1`,
  `due = now + S[step+1]`, or **graduate** if that was the last step; Easy →
  graduate as Easy. Graduating from `new`/`learning`: `kind = review`,
  `interval` = graduating interval (Good) or easy interval (Easy). Graduating
  from `relearning`: `kind = review`, `interval` stays as set by the lapse.
  A `new` card answered becomes `learning` at step 0 first (then the rule
  above applies).
* `review`: Again → `lapses += 1`, `ease -= 0.20`, `interval = max(1, interval
  × 0.5)`, `kind = relearning`, `step = 0`, `due = now + relearning S[0]` (if
  relearning steps are empty: stay `review`, `due = now + interval`).
  Hard → `interval = max(interval + 1, interval × 1.2)`, `ease -= 0.15`.
  Good → `interval = max(interval + 1, interval × ease)`.
  Easy → `interval = max(interval + 1, interval × ease × 1.3)`, `ease += 0.15`.
  Then clamp to the maximum interval; **fuzz** intervals ≥ 3 days by up to ±5%
  with a deterministic value from (card id, reps) so equal cards spread out;
  `due = now + interval days`; `reps += 1`.
* **Day boundary:** a "day" starts at 04:00 local time. "Due today" means
  `due <` the next 04:00. Reviews and new cards answered are counted per day for
  the daily limits.
* **Study queue** at `now`: (1) learning/relearning cards with `due ≤ now`, by
  due; (2) review cards due today, by due, up to the reviews left today;
  (3) new cards up to the new left today, in the deck's new card order. Order:
  learning first; reviews and new cards interleaved (one new after every
  `max(1, reviews / new)` reviews). After each answer the queue is recomputed,
  so a card that becomes due within the session comes back. When nothing is
  due but a learning card is due within 20 minutes, show it early ("learn
  ahead"). Otherwise the session ends.
* **New card presentation (Learn):** the first time a new card is shown, each
  trainee move is first shown as an arrow and the user plays it (no mistakes are
  possible except a wrong move, which just shows the arrow again); then the
  card is answered as Good automatically (moving to learning step 1 per the
  rules above — i.e. it comes back after 10 minutes, then graduates).

### Stats

Per deck: total, new, learning, due today, suspended, mature (interval ≥ 21
days), reviews answered today, retention over the last 30 days (share of
review-card answers that were not Again), and a 7-day due forecast.

## Global Constraints

- Branch `exp` only; commit after each task; imperative subject ending with
  `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. No push, no PR.
  Leave untracked `.claude/` and `exp-output/` alone.
- Decks live only in the app data folder; source PGN files and databases are
  only read. Deleting a deck asks first.
- Match surrounding style (`replay_training.rs` store, `AppError` codes,
  `blocking(...)`, decimal strings for large counts in the frontend API,
  `ReplayTrainingSession.tsx` board input, dialogs' accessible names).
- Time comes from the frontend (`now` ms and the local UTC offset in minutes)
  in every command that schedules, so tests are deterministic.
- New docs files go on the allow-lists in `scripts/check-distribution.mjs` (and
  `tests/distribution.test.ts` where listed); this plan is already on it.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
  `cargo fmt --all --check`, `npm run typecheck`, `npm test`,
  `npm run check:distribution` pass at the end of every task.

## Review Focus

- The scheduler matches the rules exactly (tests per transition).
- Day boundary at 04:00 local with the offset given; daily limits counted
  per day.
- Card ids stable across re-imports (update from source keeps progress).
- Mistake vs "another line" handling; a mistake always ends in Again.
- Atomic deck writes; a corrupt or newer-version deck file is reported, never
  overwritten silently.

---

### Task 1: Deck model and line extraction

**Files:** new `src-tauri/src/srs/mod.rs`, `src-tauri/src/srs/deck.rs`,
`src-tauri/src/lib.rs`

- [ ] Types (serde camelCase): `Deck { version: u32 (=1), id, name, created, source: String, options: DeckOptions, cards: Vec<Card>, states: BTreeMap<String, CardState>, log: Vec<ReviewLog> }`,
  `Card { id, start_fen, moves: Vec<CardMove { uci, san, comment: Option<String> }>, trainee: Colour, white, black, event, date, source_label, order: u32, suspended: bool }`,
  `DeckOptions` with the defaults above plus `include_variations`, `trainee: TraineeChoice { SideToMove, White, Black }`, `new_order: NewOrder { Source, Random }`.
- [ ] `pub fn lines_of(game: &chess_core::Game, include_variations: bool, trainee: TraineeChoice) -> Vec<Card>` (id, order filled by the caller).
- [ ] Failing tests first: a puzzle game with FEN (Black to move) → one card,
  trainee Black; a repertoire game with two variations → three cards with the
  right paths; trailing opponent move dropped; a path without trainee moves
  skipped; ids stable and equal for equal lines; comments kept per move.
- [ ] Commit.

### Task 2: Scheduler and queue

**Files:** `src-tauri/src/srs/schedule.rs`

- [ ] `pub fn answer(state: &CardState, grade: Grade, now_ms: i64, opts: &DeckOptions, card_id: &str) -> CardState`
  exactly per Scheduling. `pub fn day_start(now_ms, utc_offset_min) -> i64`
  (the latest 04:00 local ≤ now).
- [ ] `pub fn queue(deck: &Deck, now_ms, utc_offset_min) -> Queue { cards: Vec<String>, new_left, reviews_left, learning_due, review_due, next_learning_due: Option<i64> }`
  per the Study queue rules (counting today's answers from the log).
- [ ] `pub fn stats(deck: &Deck, now_ms, utc_offset_min) -> DeckStats`.
- [ ] Failing tests first: each transition in Scheduling (new→learning→review
  with Good, Easy graduation, Hard on last step ×1.5, review Again → relearning
  with ease and interval, Hard/Good/Easy interval maths with the +1 floor, ease
  floor 1.3, max interval clamp, fuzz within ±5% and deterministic); day start
  across the 04:00 boundary with a non-zero offset; daily new and review limits;
  interleaving order; learn-ahead within 20 minutes; source vs random new order
  (random seeded from the deck id so it is stable).
- [ ] Commit.

### Task 3: Store and commands

**Files:** `src-tauri/src/srs/store.rs`, `src-tauri/src/commands.rs`,
`src-tauri/src/lib.rs`, `src-tauri/src/registry.rs` if needed, new
`src-tauri/tests/srs.rs`

- [ ] Store in `<app data>/srs/decks/<id>.json`, set up like
  `replay_training.rs` (`set_dir`), atomic replace, a lock per deck. A file that
  does not parse or has `version > 1` is listed as unreadable (with its file
  name) and never written.
- [ ] Commands (all `blocking`):
  * `srs_decks(now, utc_offset)` → summaries (id, name, source, card count, stats);
  * `srs_create_from_pgn(path, name, options)` → summary + `skipped` (games that
    failed to parse, cards over the limit);
  * `srs_create_from_rows(database_id, ranges, name, options)` → same, reading
    games with a separate database handle like `export.rs`;
  * `srs_update_from_pgn(deck_id, path)` / `srs_update_from_rows(...)`: re-read
    the source; keep states of cards whose ids still exist; add new cards;
    suspend cards that disappeared (and report counts);
  * `srs_deck(deck_id)` → cards and options; `srs_set_options(deck_id, options)`;
    `srs_rename(deck_id, name)`; `srs_delete(deck_id)`;
  * `srs_queue(deck_id, now, utc_offset)` → the queue with the next card's full
    data and, for each position along it, the `legalUci` of the trainee's turn
    and the "other lines" moves (from all cards of the deck at that FEN);
  * `srs_answer(deck_id, card_id, grade, mistakes, hints, millis, now, utc_offset)`
    → the new state and the next queue (one round trip per card); it appends to
    the log;
  * `srs_suspend(deck_id, card_id, suspended)`.
- [ ] Tests in `src-tauri/tests/srs.rs` with a temp app-data dir: create from a
  PGN text file (puzzles with FEN, a repertoire with variations), queue order,
  answers move states and persist across a new store instance, update keeps
  progress and suspends removed lines, unreadable/newer files are reported and
  left untouched, create from rows on a temp `.2cbh` built from PGN.
- [ ] Commit.

### Task 4: Decks dialog and Study view (frontend)

**Files:** `src/api/client.ts`, `src/api/types.ts`, `src/commands/catalogue.ts`,
`src/App.tsx`, new `src/components/SrsDecks.tsx`, new `src/components/SrsStudy.tsx`,
new `src/state/srs.ts` (pure helpers, e.g. playing a card move by move),
`src/styles/shell.css`, `src/dev/demoApi.ts`, `src/tests/harness.tsx`, new
`src/tests/srs.test.tsx`

- [ ] Command `srs-decks`, label **"Spaced Repetition"**, tip "Train PGN lines
  and positions with spaced repetition", an icon (lucide `Repeat` or
  `CalendarClock`), `views: both`, in the database ribbon next to Replay
  Selected Games / Build Mate Training and in the game window's training group.
  Also reachable from the command palette.
- [ ] **Decks dialog** (accessible name "Spaced Repetition"): a table of decks
  (Name, Cards, New, Learning, Due, Retention) with **Study**, **Options**,
  **Update from source**, **Delete** per deck; **New deck from PGN…** (file
  picker) and **New deck from list** (database window: "All games in the list
  (N)" / "Selected games (M)"); creation options: Name, Trainee (Side to move /
  White / Black), Include variations, New card order. Unreadable deck files are
  listed with their file names and a note. Options dialog edits the deck
  options (steps as comma-separated minutes, validated).
- [ ] **Study view** (a modal sized like the replay session, accessible name
  "Study <deck name>"): the board turned for the trainee; the card's
  source label and players; counts (New · Learning · Due) at the top; a **Learn**
  badge on new cards with arrows for each trainee move; on a mistake the arrow
  and "Not this line's move"; on "another line" the note; comments shown once
  their move is played; after the line: **Again / Hard / Good / Easy** with the
  next interval under each (computed by a `srsPreview` command or the same
  rules mirrored in `src/state/srs.ts` — your call, but they must agree with
  Rust; prefer a backend preview to avoid duplicating the scheduler); keys 1–4
  and Enter; **Hint** (shows the arrow; caps the grade at Hard); **Suspend
  card**; **End session** with a summary (cards studied, Again count, time).
- [ ] Demo API stubs so `npm run dev` (`/?demo`) can create a demo deck of a few
  mate puzzles and study it.
- [ ] Failing tests first: open the dialog from the command; create from PGN
  sends the path and options; the deck table shows counts; Study plays the
  opponent's first move automatically, accepts the right trainee move, shows the
  mistake arrow and forces Again after a wrong move, shows the "another line"
  note without a mistake; grade buttons call `srsAnswer` with the grade; Hint
  disables Good/Easy; keys 1–4 work; End session shows the summary.
- [ ] Commit.

### Task 5: Docs

**Files:** new `docs/spaced-repetition.md`, `docs/training.md` and
`docs/mate-training.md` (links), `feature_checklist.md`, `README.md` (one
clause), distribution allow-lists

- [ ] `docs/spaced-repetition.md`: decks and cards, trainee colour, variations,
  accepted moves, the full scheduling rules and defaults, day boundary, queue,
  learn presentation, stats, storage and update-from-source; a "Mate chains"
  tip: use source order so mate in 1 is learnt before mate in 2.
- [ ] Commit.

### Task 6: Verification (report output; do not fake it)

- [ ] All commands from Global Constraints — paste the summary lines.
- [ ] Create a deck from `exp-output/mate-training/pgn/Mate chains.pgn` (untracked, in the repository folder)
  with a small Rust example or test-only binary against a temp app-data dir
  (never the user's real app data), print its stats and the first 10 queue
  entries (source order), answer a few cards with fixed `now` values and print
  the resulting states — paste the output.
- [ ] Browser preview check: `npm run dev`, open `http://localhost:1420/?demo`,
  open Spaced Repetition, create the demo deck, study two cards (one correct,
  one with a mistake), and describe what was shown.
