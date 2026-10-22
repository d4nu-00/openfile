# Final Material Search and PGN Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make this task possible from the GUI: "Make a `.pgn` of 100 games whose
final position has equal pieces, counting bishops and knights alike — 50 White
wins and 50 Black wins."

**Why the app cannot do it today (checked 2026-10-22 on branch `exp`):**

1. The search mask has no material criterion. Game data → Final position only
   offers Mate / Stalem. / Check (`crates/cbh-reader/src/search.rs`,
   `GameFilter::matches_game`). `feature_checklist.md` line 93, "Material search",
   is unchecked.
2. PGN export only exists for one open game (`save_document` in
   `src-tauri/src/documents.rs`, `editing.ts` `save()`). There is no way to
   write the games of a list selection to a `.pgn` (`feature_checklist.md`
   line 39 is unchecked).

Create database, Copy (Ctrl+C) and Paste (Ctrl+V) of list rows already exist,
so once (1) and (2) exist the GUI flow is:

1. Open the big database. Search mask → Result 1-0 + Final material "Equal
   pieces" (Bishop = Knight on) → Search. Click row 1, shift-click row 50,
   Copy.
2. New Database `EqualMaterial.2cbh` → Paste.
3. Back in the big database: search again with Result 0-1 → select 50 → Copy →
   paste into `EqualMaterial.2cbh`.
4. In `EqualMaterial.2cbh`: Select All → **Export PGN…** → `equal-material-100.pgn`.

**Architecture:** (A) a `final_material` criterion on `GameFilter`, evaluated
on the main line's final position in the same game-reading pass as Mate/Check;
(B) a reusable `cbh_reader` function that writes the games of a list of
ordinals as PGN, a Tauri command that exports the selected list rows through
it, and a ribbon command in the database window.

**Tech Stack:** Rust (cbh-reader, chess-core, shakmaty), Tauri commands,
React + TypeScript, Vitest.

## Definitions (fixed — do not reinterpret)

**Equal pieces** in a final position: White and Black have the same number of
queens, the same number of rooks, and the same number of minor pieces. With
`separateMinors` off (the default) bishops and knights are one class
(B ~ N), so B vs N is equal; with it on, bishops and knights are compared
separately. Pawns are **ignored** unless `countPawns` is on, in which case pawn
counts must be equal too. Kings are always one each. The final position is the
end of the main line (the same node `search::ending` uses). Games that cannot
be decoded never match.

## Global Constraints

- Work on branch `exp` only. Commit after each task with a clear message ending in
  `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. Do not push, do not open a PR.
- Source databases are read-only: export never writes beside or over a source
  database's files (reuse `save::reject_source_destination`).
- Match surrounding code style: comment density, naming, `AppError` codes,
  `blocking(...)` wrappers, progress `Channel`s, decimal-string counts in the
  frontend API.
- Every existing test must still pass. Don't reformat unrelated code.
- Never commit games from the user's databases (Mega Database etc.) into the repository.

## Review Focus

- B vs N counts as equal by default; Q vs R+R is never equal; pawns ignored by default.
- The material criterion is evaluated only after the cheap index fields pass
  (it must sit in `matches_game`, and `reads_games` must include it).
- Export writes exactly the selected rows, in list order, through the current
  search/sort (use `ordinals_of_rows`), and a cancelled export leaves no
  partial file (write to a temp file beside the target, then rename).
- Annotations PGN cannot hold (critical-position marks, training questions) are
  left out and counted, after one confirmation — never a hard failure per game.

---

### Task 1: Final material criterion (backend)

**Files:** `crates/cbh-reader/src/search.rs` (+ its tests module or
`crates/cbh-reader/tests/…`, follow existing test layout)

- [ ] Add to `GameFilter`:
  ```rust
  /// The main line's final position has equal pieces (see docs/search.md).
  pub final_material: FinalMaterial,
  ```
  and
  ```rust
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
  #[serde(rename_all = "camelCase", default)]
  pub struct FinalMaterial {
      /// Both sides have the same queens, rooks and minor pieces.
      pub equal: bool,
      /// Compare bishops and knights separately instead of as minor pieces.
      pub separate_minors: bool,
      /// Pawn counts must be equal too.
      pub count_pawns: bool,
  }
  ```
- [ ] Add `pub fn final_position(game: &chess_core::Game) -> Result<shakmaty::Chess, CbhError>`
  (factor the main-line walk out of `ending`, and make `ending` use it).
- [ ] Add `impl FinalMaterial { pub fn matches(&self, board: &shakmaty::Board) -> bool }`
  implementing the Definitions above.
- [ ] Write failing unit tests first (FEN boards) for: B vs N equal by default and
  unequal with `separate_minors`; B+B vs N+N likewise; Q vs R+R unequal; extra
  pawn ignored by default and unequal with `count_pawns`; bare kings equal;
  R vs B unequal.
- [ ] Wire it in: `reads_games()` includes `final_material.equal`;
  `matches_game` decodes the moves once (share the decoded game with the
  ending check rather than decoding twice) and rejects when unequal.
- [ ] Add an integration-style test that searches a small database (build one in a
  temp dir the way existing cbh-reader tests do, e.g. from PGN text via the
  `pgn_to_2cbh` path / v2 writer) containing a game ending B-vs-N and one
  ending a piece up, and checks only the first matches.
- [ ] `cargo test -p cbh-reader` passes. Commit.

### Task 2: Final material in the search mask (frontend)

**Files:** `src/api/types.ts`, `src/components/SearchMask.tsx`,
`src/tests/search.test.tsx`, `src/styles/*.css` only if needed

- [ ] `GameFilter` gains `finalMaterial?: { equal: boolean; separateMinors: boolean; countPawns: boolean }`.
- [ ] Game data tab: a new row right after "Final position", legend
  **"Final material"**, with checkboxes **"Equal pieces"**, **"Bishop = Knight"**
  (checked = `separateMinors: false`; checked by default) and **"Count pawns"**.
  The last two are disabled while "Equal pieces" is unchecked. Accessible
  names exactly as quoted (tests use them).
- [ ] Failing test first: ticking "Equal pieces" and Result 1-0 and pressing
  Search calls `api.searchGames` with `finalMaterial: {equal: true, separateMinors: false, countPawns: false}`
  and `results: ["1-0"]`; unticking "Bishop = Knight" sends `separateMinors: true`.
- [ ] Make sure clearing / resetting the mask resets the new fields, like the others.
- [ ] `npm run typecheck && npm test` pass. Commit.

### Task 3: PGN export of games (backend)

**Files:** new `crates/cbh-reader/src/pgn_export.rs` (export it from `lib.rs`),
new `crates/cbh-reader/examples/export_pgn.rs`, `src-tauri/src/clipboard.rs`
(or a new `src-tauri/src/export.rs` if that reads better), `src-tauri/src/commands.rs`,
`src-tauri/src/lib.rs`

- [ ] `cbh_reader::pgn_export::write_games(db: &mut Database, ordinals: &[u64], out: &mut impl std::io::Write, cancel: &AtomicBool, progress: impl FnMut(u64, u64)) -> Result<ExportSummary, CbhError>`
  where `ExportSummary { written, skipped, stripped, cancelled }` (u64s/bool).
  For each ordinal: `db.load(o)`; no game → `skipped += 1`; otherwise clear
  `critical` and `training` on every node if any are set (`stripped += 1`
  per game affected), `chess_core::write_pgn(&game)`, write it followed by a
  blank line. A `write_pgn` error on one game counts as skipped, it does not
  abort the export. Check `cancel` between games.
- [ ] Rust test: build a small database (as in Task 1), export 2 of 3 ordinals,
  parse the output with chess-core's PGN reader and check the count, order and
  `Result` tags; a game with a training question is written with
  `stripped == 1`.
- [ ] Example `export_pgn`: `export_pgn --path <db> --filter '<GameFilter JSON>' --limit N --out file.pgn`
  — runs `db.search`, takes the first N hits, calls `write_games`. (Used for
  acceptance; mirrors `examples/search.rs`.)
- [ ] Tauri command `export_rows_pgn(database_id, ranges: Vec<RowRange>, path, events: Channel<{done,total}>) -> ExportResponse { written, skipped, stripped, cancelled, destination }`
  (counts as decimal strings like `PasteResponse`). Use
  `ordinals_of_rows(&database_id, &ranges, MAX_EXPORT)` with
  `MAX_EXPORT = 1_000_000`; open a separate read handle on the database path
  (like `paste_games` does for the source) so the list stays usable;
  `save::reject_source_destination`; write to `<path>.part` then rename; delete
  the `.part` on error or cancel. Add `cancel_export(database_id)` using the
  same cancel-map pattern as `paste_cancels`. Register both in `lib.rs`.
- [ ] `cargo test --workspace` passes. Commit.

### Task 4: Export PGN command (frontend)

**Files:** `src/api/client.ts`, `src/api/types.ts`, `src/commands/catalogue.ts`,
`src/App.tsx`, `src/dev/demoApi.ts`, `src/tests/harness.tsx`,
`src/tests/clipboard.test.tsx` (or a new `src/tests/exportPgn.test.tsx`)

- [ ] Client: `chooseSavePgn(defaultName: string)` (save dialog, PGN filter),
  `exportRowsPgn(databaseId, ranges, path, onProgress)` and `cancelExport(databaseId)`;
  mock them in `harness.tsx`, and give `demoApi.ts` a stub that returns a
  plausible response so the browser demo (`npm run dev`) works.
- [ ] Catalogue: command `db-export-pgn`, label **"Export PGN"**, tip "Write the
  selected games to a PGN file", a lucide icon (e.g. `FileDown`), `views: db`,
  placed in the database ribbon's Home › Copy group after `db-select-all`.
  Enabled when a database is open, `w.rowCount > 0` and no export is running.
- [ ] App: run = take the current list selection ranges the same way
  `copyGames()` does; one confirmation (`ask`) "Export N games as PGN?
  Critical-position marks and training questions, which PGN cannot hold, are
  left out."; `chooseSavePgn("games.pgn")`; call `exportRowsPgn` with status-bar
  progress and a Cancel link like paste; finish with status
  `Exported {written} games to {destination}` plus `, {skipped} skipped` /
  `, annotations left out of {stripped}` when non-zero. Report errors like other commands.
- [ ] Failing test first: select rows 1–3 in the harness, run Export PGN, confirm,
  and check `exportRowsPgn` got the selection ranges and the chosen path, and the
  status text shows the written count. Also: cancelling the save dialog does nothing.
- [ ] `npm run typecheck && npm test` pass. Commit.

### Task 5: Docs

**Files:** `docs/search.md`, `docs/game-list.md`, `feature_checklist.md`, `README.md`

- [ ] `docs/search.md`: add "Final material" to the game-data table and a short
  paragraph with the Definitions above (workbench's own design; the reference
  app's Material tab was not traced).
- [ ] `docs/game-list.md`: an "Export PGN" section (selection, order, what is
  left out, `.part` + rename, source files protected).
- [ ] `feature_checklist.md`: mark line 39 partial *(selected rows as PGN; no
  engine-comment choice yet)* and line 93 partial *(final position equal
  pieces, B = N option; no endgame-type ranges yet)* in the style used elsewhere.
- [ ] README "PGN in and out" / Search bullets: one clause each.
- [ ] Commit.

### Task 6: Final verification (report output, do not fake it)

- [ ] `cargo test --workspace`, `npm run typecheck`, `npm test` — paste the summary lines.
- [ ] Acceptance on a real database (read-only, write output to the scratchpad given in the prompt, NOT the repo):
  ```sh
  cargo run --release -p cbh-reader --example search -- --path "<db>" --filter '{"results":["1-0"],"finalMaterial":{"equal":true},"yearMin":2015}'
  ```
  and the same for `0-1`; report hit counts and timing.
