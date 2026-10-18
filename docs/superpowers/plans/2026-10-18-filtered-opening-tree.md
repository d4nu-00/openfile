# Filtered Opening Tree Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show opening move statistics from the current search result in the Book pane.

**Architecture:** The backend already keeps search hits per database. A new cancellable query scans at most 50,000 hits and uses the existing main-line statistics collector for the board position. The Book pane selects the database tree or the filtered result; the filtered response states exactly how many hits were scanned.

**Tech Stack:** Rust/Tauri, cbh-reader, React/TypeScript, Vitest.

**Spec:** [Additional OpenFile workflows](../../../feature_checklist.md#17-additional-openfile-workflows), first item. The user's approved feature description is the source of intent.

## Global Constraints

- Keep source databases read-only for this query.
- Never present a sampled result as complete; show scanned and total hit counts.
- A new board position, search result, or hidden pane must cancel obsolete work.
- Tree statistics use the main line, as the existing database tree does.

## Review Focus

- Empty search result: show zero moves without an error.
- Very large result: scan only the first 50,000 hits and label the sample.
- Transposed position: count games whose main line reaches the board by any move order.
- Database or search switched during a request: do not show stale statistics.
- Search result absent: the filtered query returns a clear error and the toggle is unavailable.

---

### Task 1: Backend filtered statistics

**Files:** `src-tauri/src/trees.rs`, `src-tauri/src/registry.rs`, `src-tauri/tests/trees.rs`

**Interfaces:** `Registry::filtered_opening_tree(database_id: String, line: Line, request_id: String) -> Result<TreeResponse, AppError>`; `Registry::cancel_filtered_opening_tree(database_id: String, request_id: String) -> Result<(), AppError>`. `TreeResponse` gains `sampled_hits` and `total_hits` optional fields; filtered responses use `TreeStatus::Filtered`.

- [x] Write backend tests for filtered counts, empty result, missing search, a transposition, and a >50,000-hit sample.
- [x] Run `cargo test -p openfile --test trees --offline` and observe the feature tests fail.
- [x] Implement the query using the active `Hits`, `Database::stats_of_games`, and a cancellation token; keep the database handle separate from list reads.
- [x] Run the focused test to green.

### Task 2: IPC and frontend contract

**Files:** `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/api/client.ts`, `src/api/types.ts`, `src/tests/tree.test.tsx`

**Interfaces:** `WorkbenchApi.filteredOpeningTree(databaseId, line, requestId)` and `cancelFilteredOpeningTree(databaseId, requestId)`.

- [x] Write a UI test that selects the filtered tree and expects the filtered API request.
- [x] Run `npm test -- src/tests/tree.test.tsx` and observe the feature test fail.
- [x] Add the Tauri commands and typed client methods.
- [x] Run the focused UI test to green.

### Task 3: Book pane toggle and lifecycle

**Files:** `src/state/positionPanes.ts`, `src/components/BookPane.tsx`, `src/App.tsx`, `src/tests/tree.test.tsx`, `docs/opening-tree.md`, `feature_checklist.md`

**Interfaces:** The Book pane receives the current search context and a database/search-results scope selector. `usePositionPanes` asks the selected endpoint and cancels obsolete filtered queries.

- [x] Write UI tests for switching scopes, sample label, returning to the full tree, and stale-response rejection.
- [x] Run focused tests and observe the intended failures.
- [x] Implement the selector, request lifecycle, and truthful display of `sampledHits`/`totalHits`.
- [x] Update the opening-tree documentation and checklist status after the feature works.
- [x] Run `npm test`, `npm run build`, and `cargo test -p openfile --test trees --offline`; inspect the output and diff.
