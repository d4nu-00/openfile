# Playing Style Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deliver a useful, transparent local STYLE_REPORT and verify it with two real players in the native app.

**Architecture:** A bounded reader selection scans headers once. Rust replays sampled main lines into explicit descriptive metrics. React shows criteria, progress, evidence and colour splits, guarded by unique job identities.

**Tech Stack:** Existing Rust, shakmaty, Tauri, TypeScript and React; no new dependencies.

**Spec:** docs/superpowers/specs/2026-10-20-playing-style-design.md

## Global Constraints

Read-only reference databases. Preserve unrelated local and research state. Independently implement, clearly separating reference findings from design choices. No private services. All claims need measured evidence.

## Review Focus

- Reopening/rerunning with the same player must reject older jobs.
- Unknown metadata and zero observations must have distinct displays.
- Filtering must precede sample limiting, including explicit aliases.
- Partial/unsupported move data must not quietly become complete games.
- Evidence nodes must belong to the exact source snapshot analysed.

### Task 1: Reference investigation and bounded backend

Files: reader selection and tests; src-tauri/src/playing_style.rs and integration tests; registry lifecycle and command registration.

- [x] Inspect reference tooling/prior evidence; run a fresh REA/Ghidra investigation and record addresses, controls, formulas and unknowns.
- [x] Write failing synthetic selection/metric tests, run them, implement the bounded scan and main-line measures, then make them pass.
- [x] Test both colours, aliases/deleted/filter/sample edge cases, unknowns, setups/promotions/corruption and cancellation/source changes.

### Task 2: Native report flow

Files: src/components/PlayingStyleDialog.tsx; API types/client, App, command catalogue, CSS, demo and test harness.

- [x] Write failing UI tests for generation, criteria, evidence, insufficient samples and stale/cancelled jobs.
- [x] Implement criteria and report views with generation guards; wire supporting entries/positions into game navigation.
- [x] Run focused tests and type checks.

### Task 3: Acceptance and delivery

Files: docs/playing-style.md, docs/validation.md, docs/media, feature_checklist.md, README.md and source allowlist.

- [x] Verify real native Carlsen and Nakamura reports, independently check metrics and capture/inspect screenshots.
- [x] Measure large-database selection and replay costs.
- [x] Run required Rust/frontend/distribution checks and native build.
- [x] Obtain a focused independent review, fix findings and rerun relevant checks.
- [x] Commit, push, open and attach unmerged [PR #46](https://github.com/d4nu-00/openfile/pull/46).
- Delivery requires monitoring the PR checks and fixing failures before finishing; the PR retains the current cross-platform CI record.
