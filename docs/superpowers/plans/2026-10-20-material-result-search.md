# Material and result search implementation plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan inline, with test-first changes and a final independent review.

**Goal:** Complete material criteria and replace/refine/add search modes from checklist §4.

**Architecture:** Material predicates replay candidate games after indexed game-data filters. Scoped searches capture immutable entry identities, scan a separate reader, then atomically publish a bounded result only if still current. The existing list sort remains a presentation layer.

**Tech stack:** Rust reader and registry, Tauri commands, React/TypeScript, synthetic Rust and UI tests.

**Spec:** User request and the rules/evidence recorded in docs/search.md and docs/position-search.md.

## Global constraints

- Start from freshly fetched origin/main; preserve the original checkout.
- Keep reference databases read-only; independently implement behavior without proprietary code/assets.
- All games replaces results; Within results intersects by entry identity; Add unions without duplicates.
- Preserve prior results on cancellation, errors, reset or supersession. Keep deterministic entry order and active column sorting.
- Bound result size, support progress/cancel and avoid replaying excluded games during refinement.

## Review focus

- Empty and absent result sets must differ; never silently search all games for an empty refinement.
- Duplicate entries and existing hit nodes must stay aligned after union and sorting.
- Reset, newer searches and database closure must prevent stale publication.
- Promotions and setup positions invalidate assumptions based on original piece counts.
- Limit handling must not discard the previous result during Add.

### Task 1: Material matching and selective scans

**Files:** crates/cbh-reader/src/{material,search,scan}.rs; crates/cbh-reader/tests/material.rs.
**Interfaces:** MaterialQuery compiled into a validated replay predicate; Database::search_selected(filter, selection, limit, cancel, progress); existing search remains a whole-database wrapper. SearchResult carries optional found nodes.

- [x] Add synthetic tests for endgame piece ranges, signed balance, colour reversal, move windows, length, variations, promotion, validation, selection, limits and cancellation. Run them and observe missing behavior.
- [x] Implement independent material matching and parallel selected-entry scanning; preserve existing indexed filtering.
- [x] Run reader tests and static checks.

### Task 2: Atomic result combination

**Files:** src-tauri/src/{searches,commands}.rs; src-tauri/tests/searches.rs; position reader selection support.
**Interfaces:** SearchScope all/within/add; scoped registry methods retain compatibility wrappers; optional scope on search command requests.

- [x] Add synthetic integration tests for intersection/union, sorting, nodes, empty/absent results, error/cancel/reset/supersession, and limits.
- [x] Implement immutable snapshots, sparse refinement, unique deterministic union and publication ownership checks.
- [x] Run registry and reader tests.

### Task 3: UI and delivery

**Files:** src/components/{SearchMask,MaterialMask}.tsx; src/api; src/state/workbench.ts; src/tests; src/dev/demoApi.ts; docs/search.md; docs/position-search.md; feature_checklist.md.
**Interfaces:** Material fields on GameFilter; explicit search target on both game and position flows; existing quick search defaults to All.

- [x] Add UI tests for material controls/validation, scope selection, reset, progress/cancel and retained results; run them red.
- [x] Implement material preset/range/balance controls and explicit target selection with no-results modes disabled.
- [x] Verify actual running UI with synthetic database, inspect/show screenshots, measure large read-only database searches.
- [x] Record reference controls/addresses, uncertainties, design choices, measurements and limitations; tick checklist only when verified.
- [x] Run full local checks, obtain independent review, fix findings and open an unmerged PR.
- [ ] Inspect/fix CI failures and confirm the final cross-platform checks.


## Execution record

- Isolated worktree from fetched origin/main 82a82dc; branch codex/material-result-search.
- Material tests observed red before implementation, then passed with synthetic setup/capture/promotion/variation games. Selected scans and fixed-orientation duration are covered.
- Scope integration tests observed missing API failures before implementation; bounded union test reproduced and fixed an ordering defect.
- Independent code review found preset-reset display mismatch, hidden material validation blocking position-only queries, and booster cleanup erasing newer tokens. All reproduced red and fixed; reviewer approved the core code.
- Browser and native UI checked. After the Mac was unlocked, native real-reader flows verified All, Within, Add, repeated Add, matching-node opening, large-database progress/cancel preserving prior results, and Within on an empty result. Native screenshots inspected and saved.
- Release measurements on both read-only reference formats recorded in docs/search.md.
- Final checks and PR/CI evidence recorded in docs/validation.md.

- PR #45 opened unmerged. Initial macOS CI exposed an existing review-practice drag-test timing race, reproduced locally on attempt 4 and fixed by awaiting initial render/position effects; 20 repeated targeted runs passed. Cross-platform checks rerun after this test-only fix.
