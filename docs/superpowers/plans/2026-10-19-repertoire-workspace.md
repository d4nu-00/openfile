# Repertoire Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A persistent White and Black repertoire overlay with board editing, coverage and practice.

**Architecture:** A Rust store owns two JSON files in app data and resolves position identity through shakmaty. A React tab uses the current game board, opening tree and local store endpoints. Practice grading is backend validated and stored separately.

**Tech Stack:** Rust, Tauri 2, React, TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-10-19-repertoire-workspace-design.md`

## Global Constraints

- Keep the workspace independent of source databases and game annotations.
- Use position identity with legal chess state; White and Black are separate.
- Persist edits with atomic replacement; practice uses a separate file.
- Label missing, limited and sampled reference data.

## Review Focus

- A transposed position shares choices for one colour.
- The same position in the other colour stays separate.
- A stale move remains removable and cannot count as a correct practice answer.
- Invalid paths cannot write corrupted state.
- An unavailable tree does not imply full coverage.

---

### Task 1: Persistent repertoire and practice model

**Files:** Create `src-tauri/src/repertoire_workspace.rs`; modify `src-tauri/src/lib.rs`.

**Interfaces:** `RepertoireWorkspace::snapshot(line, side)`, `edit(line, side, move, action, note)`, `grade(line, side, move)`; Tauri commands expose these to the UI.

- [x] Write tests for transpositions, colour separation, restart persistence, invalid moves and practice grading.
- [x] Run them and confirm feature failures.
- [x] Implement the store with canonical legal FEN keys and atomic writes.
- [x] Run the focused Rust tests.

### Task 2: Repertoire tab and board flow

**Files:** Create `src/components/RepertoirePane.tsx`; modify `src/App.tsx`, `src/api/client.ts`, `src/api/types.ts`, demo and test API.

**Interfaces:** The tab consumes snapshot/edit/grade API methods and existing Book rows; board moves are intercepted only during practice.

- [x] Write UI tests for editing, coverage labels and practice flow.
- [x] Run them and confirm feature failures.
- [x] Implement the compact tab and navigation.
- [x] Run focused and full frontend tests.

### Task 3: Documentation and release checks

**Files:** Modify `README.md`, `feature_checklist.md`, `docs/opening-tree.md`, `docs/repertoire.md`, `docs/game-window.md`.

- [x] Describe the user flow, storage, transpositions, practice and limits.
- [x] Run format, lint, test, typecheck, build and distribution checks.
- [x] Inspect UI screenshots, fix defects and open an unmerged PR.
