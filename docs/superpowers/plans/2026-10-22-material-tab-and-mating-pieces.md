# Material Tab and Mating Pieces Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the user pick piece combinations in the search mask:

1. **Final material:** how many queens, rooks, bishops, knights (or minor
   pieces) and pawns each side has in the main line's final position.
2. **Mate by:** games that end in mate where the mating side's pieces
   *involved in the mate* are a chosen combination, e.g. "queen and knight,
   nothing else".

Both combine with every other search field (Result, Year, Elo …), and the
results can then be copied/exported with the existing Copy, Paste and Export
PGN commands.

**Builds on:** `docs/superpowers/plans/2026-10-22-final-material-search-and-pgn-export.md`
(already done on `exp`: `FinalMaterial` "Equal pieces" on the Game data tab,
`final_position()`, Export PGN). Leave the Equal pieces row where it is.

**Architecture:** a new `material: MaterialQuery` field on `GameFilter`,
evaluated in `GameFilter::matches_game` on the same decoded game as the other
final-position criteria; a pure function that lists the pieces involved in a
mate; a new **Material** tab in `SearchMask.tsx`.

**Tech Stack:** Rust (cbh-reader, shakmaty 0.30), React + TypeScript, Vitest.

## Definitions (fixed — do not reinterpret)

**Count range.** Each piece kind takes a `CountRange { min: Option<u8>, max: Option<u8> }`;
both `None` means "any". A count `n` matches when `min <= n <= max` for the
bounds given.

**Piece counts.** `PieceCounts` has a range for `kings`, `queens`, `rooks`,
`bishops`, `knights`, `minors` (bishops + knights together) and `pawns`, plus
`others_zero: bool`. Every range given must match. With `others_zero`, every
kind left at "any" among queens, rooks, bishops, knights and pawns must be 0 —
except that bishops and knights are not forced to 0 when `minors` is given, and
`minors` is not forced to 0 when bishops or knights are given. Kings are never
forced by `others_zero`.

**Final material.** `material.white` / `material.black` are matched against
White's and Black's pieces in the final position of the main line. With
`either_colour`, the game also matches when the two sides' counts match the
other way round. The criterion is active when any range is set or
`others_zero` is on for either side.

**Pieces involved in a mate.** Only for a final position that is checkmate.
Let M be the mated side, `s` its king's square, A the mating side.
* Escape squares E = the king-move squares from `s` that are not occupied by
  M's own pieces (empty squares and squares holding A's pieces).
* A piece of A (king and pawns included) is **involved** when it gives check
  (attacks `s` on the real board), **or** it attacks at least one square of E,
  computed with M's king removed from the occupancy (so a slider's attack
  continues through `s`; the king cannot step back along the checking line).
  Pawns attack only their two capture squares; the king its king-move squares.
* Pins, blockers and pieces that only defend other attackers from afar are not
  considered (later work, note it in the docs).
* `mate_by` is matched against the counts of involved pieces by kind (minors =
  involved bishops + involved knights). Active when `mate_by_enabled`; then a
  game that does not end in mate never matches.

Examples (all must be tests):
* Back-rank mate `6k1/5ppp/8/8/8/8/8/R5K1 b - - 0 1` after `Ra8#`
  (`R5k1/5ppp/8/8/8/8/8/6K1 b - - 0 1`): involved = {R}; White king not involved.
* Scholar's mate final position `r1bqkb1r/pppp1Qpp/2n2n2/4p3/2B1P3/8/PPPP1PPP/RNB1K1NR b KQkq - 0 4`:
  involved = {Q, B} (the bishop defends f7, the queen's square, which is an
  escape square because it holds an A piece).
* Smothered mate `6rk/5Npp/8/8/8/8/8/6K1 b - - 0 1`: involved = {N}.
* Queen + king mate `k7/1Q6/2K5/8/8/8/8/8 b - - 0 1`: involved = {Q, K}.
* Rook, king and pawn: `R6k/8/5KP1/8/8/8/8/8 b - - 0 1` (checkmate): involved
  = {R, K, P} — the rook checks and covers g8, the king covers g7, the g6 pawn
  covers h7.
* Removing the mated king from the occupancy is still required so that the
  attack sets are right, although any slider that sees through `s` also gives
  check, so it never changes which pieces are involved on its own; no separate
  test is needed for it.

## Global Constraints

- Work on branch `exp` only; commit after each task; subject line imperative,
  and end every commit message with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
  Do not push, no PR.
- Source databases are read-only. Never commit games from the user's databases.
- Match surrounding code style (comment density, naming, serde
  `camelCase`/`default`, frontend decimal strings). Don't reformat unrelated code.
- Every existing test must still pass.

## Review Focus

- The Definitions above, especially x-ray through the king square, pawns
  attacking only diagonally, and `others_zero` with minors vs bishops/knights.
- `reads_games()` includes the new criteria, and the game is decoded once
  (`matches_game` already shares one decoded game; extend it).
- An empty `MaterialQuery` (the default) costs nothing and matches everything.
- `either_colour` swaps whole sides, not individual piece kinds.

---

### Task 1: Count ranges, piece counts and final material (backend)

**Files:** `crates/cbh-reader/src/search.rs` (or a new `crates/cbh-reader/src/material.rs`
re-exported from `search` if `search.rs` grows too much — your call),
`crates/cbh-reader/tests/search.rs`

- [ ] Types (serde `camelCase`, `default`, `Clone, Copy?, Debug, Default, PartialEq, Eq, Deserialize, Serialize`):
  ```rust
  pub struct CountRange { pub min: Option<u8>, pub max: Option<u8> }
  pub struct PieceCounts { pub kings: CountRange, pub queens: CountRange, pub rooks: CountRange,
      pub bishops: CountRange, pub knights: CountRange, pub minors: CountRange,
      pub pawns: CountRange, pub others_zero: bool }
  pub struct MaterialQuery { pub white: PieceCounts, pub black: PieceCounts,
      pub either_colour: bool, pub mate_by_enabled: bool, pub mate_by: PieceCounts }
  ```
  and `pub material: MaterialQuery` on `GameFilter` (doc comment pointing at docs/search.md).
- [ ] A small `Tally { kings, queens, rooks, bishops, knights, pawns: u8 }` with
  `Tally::of(board, color)`; `PieceCounts::matches(&Tally) -> bool` and
  `PieceCounts::is_any()`.
- [ ] Failing unit tests first: ranges (any, exact, at least, at most);
  `others_zero` with and without minors; `R+B vs R+N` matched by
  `rooks=1, minors=1, others_zero` on both sides; `either_colour` with
  `White Q=1 / Black R=2` matching a position where Black has the queen.
- [ ] Wire into `reads_games()` and `matches_game()`.
- [ ] `cargo test -p cbh-reader`; commit.

### Task 2: Pieces involved in a mate (backend)

**Files:** same as Task 1

- [ ] `pub fn mating_pieces(pos: &shakmaty::Chess) -> Option<Tally>` (None
  unless checkmate) implementing the Definitions with
  `shakmaty::attacks::{attacks, pawn_attacks, king_attacks}` (check the 0.30 API
  names) and `Board::occupied()` minus the mated king.
- [ ] Failing tests first: every example in Definitions, plus a position that is
  not mate → `None`.
- [ ] `mate_by_enabled` in `reads_games()`/`matches_game()`: not mate → no match;
  else `mate_by.matches(&tally)`.
- [ ] An integration test on a small database built in a temp dir (reuse the
  helper the Task 1 integration test of the previous plan uses in
  `crates/cbh-reader/tests/search.rs`): a back-rank rook mate, a Q+N mate and a
  non-mate; `mate_by` rook-only (`rooks=1, others_zero`) finds only the first.
- [ ] Extend `crates/cbh-reader/examples/search.rs` output: for each of the
  first hits also print the involved pieces when the game ends in mate (e.g.
  `mate by Q,N,K`) — this is for acceptance.
- [ ] `cargo test --workspace`; commit.

### Task 3: Material tab (frontend)

**Files:** `src/api/types.ts`, `src/components/SearchMask.tsx`, `src/styles/*.css`
(only as needed, reuse existing mask classes), `src/tests/search.test.tsx`

- [ ] Types mirroring Task 1 (`CountRange`, `PieceCounts`, `MaterialQuery`,
  `GameFilter.material?`).
- [ ] New tab **"Material"** between "Annotations" and "Position" (tab id
  `material`). Layout:
  * Section legend **"Final position"**: a small grid with header row
    Q, R, B, N (or **Minor** instead of B and N), P, and rows **White** and
    **Black**; each cell a `<select>` with options `any`, `0`–`3` (`0`–`8` for P),
    `1+`, `2+`. Accessible names `"White queens"`, `"Black minor pieces"`,
    `"White pawns"`, etc. (`<kind> <plural noun>`; minors = `"minor pieces"`).
  * Checkboxes under it: **"Either colour"**, **"Bishop = Knight"** (checked by
    default; checked shows the Minor column and sends bishops/knights as any;
    unchecked shows B and N and sends minors as any), **"No other pieces"**
    (sets `othersZero` on both sides).
  * Section legend **"Mate by"**: checkbox **"Final position is mate by"**
    (`mateByEnabled`), then one row of selects for the mating side's involved
    pieces: K (`any`, `0`, `1`), Q, R, B/N or Minor, P, accessible names
    `"Mating king"`, `"Mating queens"`, `"Mating rooks"`, `"Mating minor pieces"`
    / `"Mating bishops"`, `"Mating knights"`, `"Mating pawns"`; and **"No other
    pieces"** for `mateBy.othersZero` (accessible name "Mate by: no other pieces"
    to differ from the material one). The Bishop = Knight checkbox above
    applies here too. Selects disabled while the checkbox is off.
  * A one-line hint like the Game data tab's slow-search note when any of it is set.
- [ ] Select value ↔ `CountRange`: `any` → `{}`, `n` → `{min:n,max:n}`,
  `n+` → `{min:n}`.
- [ ] Reset all clears the tab. Searching from this tab runs `onSearch(f)` like
  the Game data tab.
- [ ] Failing tests first: choosing Mate by Q=1, Minor=1 (Bishop = Knight on)
  and Mate-by "No other pieces" sends
  `material.mateBy = {queens:{min:1,max:1}, minors:{min:1,max:1}, othersZero:true, …any}`
  with `mateByEnabled: true`; unticking Bishop = Knight shows "Mating bishops"
  and "Mating knights"; White rooks `1+` sends `{min:1}`; Either colour sends
  `eitherColour: true`; Reset all clears.
- [ ] `npm run typecheck && npm test`; commit.

### Task 4: Docs

**Files:** `docs/search.md`, `feature_checklist.md`, `README.md` (one clause)

- [ ] `docs/search.md`: a "Material tab" section with the Definitions (ranges,
  others_zero, either colour, involved pieces incl. the x-ray rule and what is
  not considered), stating it is the workbench's own design (the reference
  app's Material tab was not traced).
- [ ] `feature_checklist.md` line for "Material search": update the partial note.
- [ ] Commit.

### Task 5: Verification (report output; do not fake it)

- [ ] `cargo test --workspace`, `npm run typecheck`, `npm test` summary lines.
- [ ] Acceptance on the real database, read-only, release build, outputs only
  in the scratchpad directory given in the prompt:
  * Mate by Q+N only (`queens 1, minors 1, othersZero`, king any), `yearMin 2015`:
    hits, time, first hits with their printed mating pieces.
  * Final material R+minor vs R+minor, pawns any, others zero, either colour,
    results `1-0`: hits and time.
  * Export the first 20 Q+N mates with the `export_pgn` example to
    `mate-QN-20.pgn` in the scratchpad.
