# Quick Search Expressions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the game-list quick filter accept useful combined expressions while preserving plain-word search.

**Architecture:** Parse quick-filter text in `cbh-reader` into typed terms, resolve name terms once against name tables, then compare terms against each entry header during the existing parallel scan. Keep the current API and search state.

**Tech Stack:** Rust, Tauri 2, React, Vitest and Cargo tests.

**Spec:** `docs/superpowers/specs/2026-10-18-quick-search-expressions-design.md`

## Global Constraints

- Database files remain read-only during search.
- Existing plain-word queries retain their matches.
- Invalid expressions fail visibly; terms are never silently dropped.

## Review Focus

- A name ending in `-` matches only White; a name beginning with `-` matches only Black.
- Ranges include both endpoints and reject reversed bounds.
- Unrated games do not match an Elo comparison.
- Quoted event/site values containing spaces stay one term.
- Cancellation and a limited hit list behave as before.

---

### Task 1: Parse and match quick expressions

**Files:** `crates/cbh-reader/src/search.rs`, `crates/cbh-reader/tests/search.rs`, `crates/cbh-reader/src/error.rs` if a query error type is needed.

- [x] Add failing integration tests for each term type, combinations, quotes, malformed input and existing plain words.
- [x] Run `cargo test -p cbh-reader --test search` and confirm expected failures.
- [x] Add the parser, name resolution and entry-header matching in the existing search path.
- [x] Re-run the targeted test and the full `cargo test -p cbh-reader` suite.

### Task 2: Explain syntax and record evidence

**Files:** `src/App.tsx`, `src/tests/gameList.test.tsx`, `docs/search.md`, `feature_checklist.md`.

- [x] Add a failing UI test showing the syntax hint and a compound query reaching the search API.
- [x] Add a concise hint to the quick-filter UI and document the supported syntax and limits.
- [x] Record the REA/Ghidra native findings and their limits in `docs/search.md`.
- [x] Mark the completed checklist item and run `npm test`, `npm run typecheck`, and relevant Cargo tests.
