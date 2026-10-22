# Native-dimension replacement plan

The initial descriptive report was delivered in PR #46. The user now explicitly requests the eight ChessBase dimensions and recovery of actual native calculations as closely as possible. This plan supersedes the initial implementation plan. Use the existing isolated worktree and PR; follow the native-dimension design in the matching spec.

- [x] Extend actual REA/Ghidra evidence for result profiles, native comparison constants, selection metadata, the six remaining composite inputs and grade behavior. Keep direct evidence, inference and unresolved dependencies separate.
- [x] Build a typed native profile/composite calculation layer with hand-calculated failing fixtures. Implement recovered Score and Decided Games first; implement additional dimensions only with complete verified input contracts. Missing dependencies must remain explicit.
- [x] Align reader selection with verified stored-length and event-category rules without dropping large-database boundedness or source checks. Cover native precedence, unknown metadata and filter-before-limit cases.
- [x] Replace the ten metric cards/API with the eight native dimensions, input explanations, raw/normalized values, availability and supporting evidence. Preserve source-bound navigation, cancellation and generation guards, including the WebKit Stop regression.
- [x] Run focused tests throughout, obtain a focused independent review and fix findings.
- [x] Rebuild and verify actual native Carlsen/Nakamura reports, independent computations, supporting navigation, cancellation and inspected screenshots. Document performance and unresolved native dependencies honestly.
- [x] Update report docs, validation and checklist; run required local checks and locked native builds.

Publication: update existing PR #46, push the final revision, monitor cross-platform CI through success and leave it unmerged. The PR records final commit/check links so this plan does not require a follow-up documentation-only CI cycle.
