# Database window and recovery implementation plan

> **For agentic workers:** Execute inline with superpowers:executing-plans; obtain a fresh independent review before delivery.

**Goal:** Complete the four database views, local database properties, and safe recovery to a new verified 2CBH family.

**Architecture:** Extend the existing start screen/settings and maintenance job patterns. Recovery reads fixed record boundaries independently of the normal database opener, retains verified raw records and blocks, and publishes a checked separate family through an exclusive whole-directory rename. Original databases remain read only.

**Tech Stack:** Rust, Tauri, React/TypeScript, Vitest.

**Spec:** The user's attached request, plus docs/start-screen.md and docs/maintenance.md. The user explicitly authorizes autonomous design and implementation without intermediate approval gates.

## Global constraints

- Start from freshly fetched origin/main in an isolated worktree; preserve all unrelated work.
- Investigate the actual reference executable through a fresh REA/Ghidra session before implementing the selected behavior; separate evidence, inference and OpenFile choices.
- No speculative classic-format writes; no proprietary assets or raw decompilation in this repository.
- Recovery never edits its source, never overwrites a destination, bounds work and diagnostics, preserves supported content or itemises omissions, verifies before publishing.
- Native UI acceptance, focused independent review, full repository checks and an unmerged PR with passing CI are required.

## Review focus

- A damaged entry count must not prevent recovery of physically bounded records, or admit arbitrary scanning.
- A missing/truncated name or annotation companion must never silently erase content.
- Stop, source mutation and superseded progress must not publish stale output.
- Destination collisions, aliases and interrupted publication must not damage either family.
- Switching views and editing properties must preserve selection and work with missing files.

## Tasks

- [x] 1. Research: trace view dispatch, property dialog/storage, and recovery handler from the binary; retain provenance in the research workspace and factual findings in product docs.
- [x] 2. Views/properties: add failing behavioral tests in src/tests/start.test.tsx and src-tauri/tests/library.rs. Implement all four views with shared selection/opening/keyboard behavior, persisted view choice, and bounded local description/type properties. Verify tests.
- [x] 3. Recovery: add synthetic damaged-family tests in crates/cbh-reader/tests/recover.rs and backend job tests. Implement bounded fixed-record recovery, raw block retention, name validation, source checks, cancellation, itemised report, output verification and safe publication. Verify tests.
- [x] 4. Product workflow: expose recovery from the start screen without opening the source, with destination selection, progress/Stop, failure/result report and explicit opening of the result. Add meaningful UI tests and verify.
- [x] 5. Acceptance: exercise every delivered feature in the native app on disposable fixtures; inspect screenshots, compare recovery output to known originals, check existing maintenance, measure time/memory/disk, update docs and checklist precisely.
- [x] 6. Review/local delivery: independent focused review, fix findings, run Rust/frontend/static/source/native checks, create and attach unmerged PR. Cross-platform CI remains a separate final delivery gate, maintained in the PR's live checks.

## Execution ledger

- Created codex/database-maintenance from origin/main 636cca1 in the managed database-maintenance worktree. Existing checkout and other worktrees unchanged.
- Baseline frontend and Rust tests passed. Rust is installed under ~/.rustup/toolchains/1.99.0-aarch64-apple-darwin/bin and must be added to PATH.
- Fresh REA session database-maintenance-20261022 analysed the requested executable with Ghidra, hash 8d633b10ad97bd9f8fbbc33a89c279397850ca759c8375093deda4a9485e8dee. Actual run slots, dialog controls, Title/Type storage, access-counter Usage and in-place repair were traced. Native view-setting storage and crash rollback remain unresolved; no standalone safe-copy command was established.
- Ruling: use lossless raw 2CBH recovery; classic recovery remains explicitly refused in this first delivery. Re-encoding through the existing conversion path would strip unrepresented annotations and metadata.
- Independent review identified three reproduced defects: lowered declared counts hiding physical records, Stop disabled before first progress, and no keyboard tab stop after changing groups. Fixed all three with regression tests. Follow-up review found no actionable remaining defect; bounded name-table verification can delay cancellation.
- Added metadata/opaque-annotation remapping, verification cancellation, source companion appearance, process interruption and actual file-size write-failure tests. Diagnostics retain 500 problems while the complete per-entry report streams to disk.
- Native acceptance used a separate app identifier/settings and disposable fixtures. Recovered 76/80 expected games; full decoded comparison and integrity passed. Verified all views/properties, restart persistence, progress/Stop, existing-destination refusal, existing integrity/doubles/compaction, and read-only browsing to entry 11,151,119. Positive native duplicate fixture marked game 2, kept game 1 and passed integrity. Twelve screenshots visually inspected in docs/media.
- Release recovery: 100,000 small games in 2.635 s, 5.42 MiB CLI peak RSS and 61.6 MB output; large read-only browsing remained bounded. Measurements are not worst-case resource claims.
- Final local checks passed: 360 Rust tests, 202 frontend tests, formatting, strict Clippy, TypeScript, Prettier, production frontend, locked native release build and source/notice audit (686 paths). One file-size failure-test harness issue caused its own redirected child log to exceed the cap; piping child output fixed the test, and the full suite then passed. Native release retained the local toolchain debug-stripping warning and existing frontend bundle advisory.
- Windows CI additionally caught general directory rename replacing an empty folder that appeared during verification. Publication now uses explicit no-replacement MoveFileW; retained the original failing end-to-end test and added a direct file/empty/populated-destination preservation test. Local checks and fresh cross-platform CI verify the fix.
- [Unmerged PR #51](https://github.com/d4nu-00/openfile/pull/51) is attached to the task. Final delivery requires success on Windows, macOS and Linux; live check results on that PR are the CI record rather than a copied status that becomes stale on each documentation commit.
