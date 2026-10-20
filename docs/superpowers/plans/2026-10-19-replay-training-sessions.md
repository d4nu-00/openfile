# Replay Training Sessions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Practise stored training questions from a game or selected list rows, and retain progress locally.

**Architecture:** A pure frontend session model validates and grades authored questions. A React modal collects positions with existing API calls and presents one question at a time. A small Tauri store persists attempts in its own app data folder.

**Tech Stack:** React, TypeScript, Vitest, Rust, Tauri, shakmaty through existing position API.

**Spec:** `docs/superpowers/specs/2026-10-19-replay-training-sessions-design.md`

## Global Constraints

- Do not alter source databases or game annotations while practising.
- Do not call legal ungraded moves engine mistakes.
- Preserve unrelated local changes and leave the PR unmerged.

## Review Focus

- A Black-to-move question uses Black's orientation and legal moves.
- Variations use the annotated node's own parent position.
- Promotions do not treat every promotion role as one authored answer.
- Closing or switching databases prevents late scan results from appearing.
- A malformed answer or changed question does not reuse old progress.

---

### Task 1: Session model

**Files:** `src/state/replayTraining.ts`, `src/tests/replayTraining.test.ts`

- [ ] Write failing tests for question extraction, variation paths, grading and promotion matching.
- [ ] Run the test to see the expected failure.
- [ ] Implement pure validation, keys, grades and summary helpers.
- [ ] Run the test and typecheck.

### Task 2: Local progress

**Files:** `src-tauri/src/replay_training.rs`, `src-tauri/src/lib.rs`, `src/api/client.ts`, `src/api/types.ts`

- [ ] Write Rust tests for restart persistence and corrupt/versioned files.
- [ ] Run the test to see the expected failure.
- [ ] Add read/write commands and app data setup with atomic replacement.
- [ ] Run the Rust test.

### Task 3: Session UI and entry points

**Files:** `src/components/ReplayTrainingSession.tsx`, `src/App.tsx`, `src/commands/catalogue.ts`, `src/styles/shell.css`, `src/tests/replayTrainingSession.test.tsx`

- [ ] Write failing UI tests for hidden answers, board input, choice input and navigation.
- [ ] Run the tests to see the expected failures.
- [ ] Implement collection, cancellation, feedback, retry, summary and ribbon commands.
- [ ] Run frontend tests and typecheck.

### Task 4: Documentation, UI inspection and integration

**Files:** `README.md`, `feature_checklist.md`, `docs/replay-training.md`, screenshots in `docs/media/`

- [ ] Update documentation and checklist.
- [ ] Run repository checks and inspect a browser screenshot; fix issues.
- [ ] Commit, push and open a PR against main; inspect CI and fix failures.
