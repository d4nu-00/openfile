# Cross-platform CBH Workbench Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Deliver a locally runnable desktop chess database workbench that reads classic CBH games, supports replay/editing/PGN export and UCI analysis, and has verified Windows, macOS and Linux builds.

**Architecture:** A Rust workspace contains independent chess, CBH and engine crates plus a thin Tauri backend. React/TypeScript consumes typed commands and events; binary parsing and legal-move validation stay in Rust. Classic CBH is paged directly from read-only files; PGN is the editable output.

**Tech Stack:** Rust stable, Tauri 2, React, TypeScript, Vite, shakmaty, pgn-reader, serde, thiserror, tempfile, Vitest and React Testing Library. Resolve compatible releases during Task 1/8, record exact Rust toolchain and both lockfiles, and use locked dependencies thereafter. Start with published shakmaty 0.30 and pgn-reader 0.29 APIs, verifying that Cargo resolves one compatible shakmaty version before writing adapters; do not use upstream unreleased `main`.

**Spec:** [Approved design](design.md).

## Global Constraints

The following requirements are copied from the approved specification:

- “Use Rust, Tauri 2 and React/TypeScript, as previously proposed.”
- “The future product lives in `app/`, independently of extracted material and analysis artifacts.”
- “Only authored source, permitted dependencies, attribution, build configuration and synthetic test fixtures enter the publishable project.”
- “Original CBH files are never rewritten.”
- “Page size is capped at 200 entries; entity caching has a fixed memory budget.”
- “The first performance checks target first-page display within two seconds and ordinary sampled-game load within one second on this Mac with local storage, after application startup.”
- “Database IDs and 64-bit offsets cross the JavaScript boundary as strings or opaque handles to avoid loss of integer precision.”
- “No engine is bundled initially.”
- “A configured CI workflow alone is not evidence of cross-platform success.”
- “GPL-3.0-or-later is the proposed project license, subject to an explicit dependency and attribution check before publishing source.”
- “`.2cbh` is a later adapter behind the same interface.”

The first stage excludes original CBA annotation import, special encodings, Chess960, decoded guiding-text content, bulk search/export and CBH writing. These remain visible compatibility limitations and follow-on work. User-created PGN comments are included.

## Review Focus

1. Companion files differing only by case, or duplicate case-insensitive matches: open the intended family or report ambiguity; never pick an arbitrary file. Pin in Task 4.
2. A database truncated or replaced after opening: return an entry-specific source-changed/read error and retain the user's working game. Pin in Tasks 3/7/9.
3. PGN tags with quotes/backslashes, comments with braces, and multiple games: preserve representable content, reject an unrepresentable edit explicitly, and expose each game. Pin in Tasks 2/10.
4. A late game result or a late UCI line after switching position: it must never overwrite the latest selection or attach analysis to the wrong board. Pin in Tasks 9/12/13.
5. Failed save, engine executable paths containing spaces, or an engine that ignores shutdown: preserve the original destination/dirty game, launch without a shell, and reap the process. Pin in Tasks 10/12.

## Execution and evidence rules

The recommended execution method is **Native**: implement tasks serially in this session, then obtain a fresh final review. The parser, game model and desktop commands share interfaces, so serial development avoids repeated context and integration churn. The alternative is a fresh implementing agent and reviewing agent per task. Wait for the user's plan review and execution-method choice before either approach.

There is currently no Git repository at the research root. At execution time, read the worktree skill, inspect attachments and initialize an independent repository at `app/` if there is still no suitable checkout. Do not initialize the research root or import its extracted material. Keep toolchain installations outside the product repository. Installing into writable workspace-local tooling paths avoids changing the user's global shell setup. Network or filesystem escalations use the available approval mechanism when required.

Copy the approved spec and this plan to `app/docs/design.md` and `app/docs/implementation-plan.md` at repository setup. In those publishable copies, replace machine-specific absolute paths with `<database-path>`/`<research-workspace>`, fix links between the copied documents, and replace parent research links with a concise factual compatibility summary. Preserve requirements and task interfaces; retain these originals as the private execution references. Maintain checkboxes and an execution log. Commit each verified task with explicitly listed paths, never `git add` the research directory. No remote repository creation, source publication, signing or paid service setup is implied.

Commands below run from `app/` unless a full path is shown. Each task has a red/green cycle: add the test, run it and establish the behavioural failure, implement only that deliverable, rerun its tests, then commit. For an absent crate, establish its manifest/module shell first; an expected compile failure can expose a missing API, but subsequent implementation must reach the meaningful assertions. Split a listed implementation step into small file-level edits and verification steps during execution; do not write the whole application before testing.

## File structure

```text
app/
  Cargo.toml, Cargo.lock, rust-toolchain.toml, LICENSE, .gitignore
  package.json, package-lock.json, tsconfig*.json, vite.config.ts, index.html
  THIRD_PARTY_NOTICES.md, README.md
  docs/{design,implementation-plan,compatibility,validation}.md
  crates/chess-core/src/{lib,model,position,pgn,error}.rs
  crates/chess-core/tests/{tree,pgn}.rs
  crates/cbh-reader/src/{lib,error,limits,source,header,companions,entities}.rs
  crates/cbh-reader/src/database.rs
  crates/cbh-reader/src/moves/{mod,tables,pieces,setup,decode}.rs
  crates/cbh-reader/tests/{headers,entities,moves,setups,database}.rs
  crates/cbh-reader/tests/support/{mod,family,encoder}.rs
  crates/cbh-reader/examples/inspect.rs
  crates/engine/src/{lib,protocol,options,session,error}.rs
  crates/engine/tests/{protocol,lifecycle}.rs
  crates/engine/src/bin/mock-uci.rs
  crates/engine/tests/support/mod.rs
  src-tauri/{Cargo.toml,build.rs,tauri.conf.json,capabilities/main.json}
  src-tauri/src/{main,lib,commands,registry,documents,save,engine_commands}.rs
  src-tauri/tests/{commands,save}.rs
  src/{main,App}.tsx
  src/api/{types,client}.ts
  src/state/{workbench,selection,editing,analysis}.ts
  src/components/{DatabaseTable,Board,NotationTree,Toolbar,Diagnostics}.tsx
  src/components/{CommentEditor,UnsavedDialog,EnginePanel,EngineOptions}.tsx
  src/styles/{layout,board}.css
  src/tests/{setup.ts,harness.tsx,browse.test.tsx,edit.test.tsx,engine.test.tsx,selection.test.tsx}
  tests/fixtures/README.md, tests/distribution.test.ts
  scripts/{compare-corpus.py,check-distribution.mjs}
  .github/workflows/desktop.yml
```

Fixture binary families are generated into temporary directories by the test-support code. Checked-in fixture PGNs are authored chess examples, not copied commercial games. Each production file above has a single responsibility; split a module if codec tables or command implementations obscure its interface.

## Shared contracts

Define these types in the owning tasks; later tasks use the exact names. All public fallible operations return the owning crate's typed error. Serde wire DTOs use camelCase fields and lowercase tagged enums. Internal IDs may be integers; serialized database ordinals and counts are decimal strings.

```rust
// chess-core/model.rs (Task 1)
pub type NodeId = u32;
pub struct Game {
    pub tags: std::collections::BTreeMap<String, String>,
    pub start_fen: String,
    pub nodes: Vec<Node>, // node 0 is the root
}
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub uci: Option<String>, // root None; analysis null move "0000"
    pub children: Vec<NodeId>, // children[0] is the main continuation
    pub comments: Vec<String>,
    pub nags: Vec<u16>,
}
pub struct PositionView {
    pub fen: String,
    pub turn: String, // "white" or "black"
    pub legal_uci: Vec<String>,
}
pub struct GameLimits {
    pub max_nodes: usize,
    pub max_variation_depth: usize,
    pub max_input_bytes: usize,
}

// cbh-reader/lib.rs (Tasks 3-7)
pub enum EntryKind { Game, Text, Unknown(u8) }
pub enum DataStatus { Available, Missing, Unsupported, Corrupt, NotDecoded }
pub struct Diagnostic {
    pub code: String, pub message: String, pub file: Option<String>,
    pub offset: Option<u64>, // convert to a string in wire DTOs
}
pub struct EntrySummary {
    pub ordinal: u64, pub kind: EntryKind, pub deleted: bool,
    pub white: Option<String>, pub black: Option<String>,
    pub event: Option<String>, pub site: Option<String>,
    pub date: Option<String>, pub result: Option<String>,
    pub white_elo: Option<u16>, pub black_elo: Option<u16>,
    pub eco: Option<String>, pub diagnostics: Vec<Diagnostic>,
}
pub struct DatabaseInfo {
    pub entry_count: u64, pub companions: Vec<CompanionInfo>,
    pub diagnostics: Vec<Diagnostic>,
}
pub struct CompanionInfo { pub extension: String, pub status: DataStatus }
pub struct EntryPage { pub entries: Vec<EntrySummary>, pub next: Option<u64> }
pub struct LoadedEntry {
    pub summary: EntrySummary, pub game: Option<chess_core::Game>,
    pub metadata_status: DataStatus, pub moves_status: DataStatus,
    pub annotations_status: DataStatus, pub diagnostics: Vec<Diagnostic>,
}
pub struct ReadLimits {
    pub max_page: usize, pub max_game_bytes: usize,
    pub game: chess_core::GameLimits, pub entity_cache_bytes: usize,
}
```

Initial defaults: page 200; game stream 4 MiB; 100,000 nodes; 128 nested variations; entity cache 8 MiB. `GameLimits::default().max_input_bytes` is 20 MiB; `ReadLimits` applies the separate 4 MiB CBG-stream cap. PGN input is capped at 20 MiB, 2,000 games and 100,000 total nodes per opened collection. Variation depth counts nested branches, not mainline plies; ordinary games longer than 128 plies must work. A limit produces a recoverable `limitExceeded` diagnostic, never silent truncation. Engine lines are capped at 16 KiB and buffered analysis events at 128, coalescing superseded `info` updates rather than blocking shutdown.

Wire requests also carry a `requestId: string`. Responses echo it. Errors serialize as `{ code, message, diagnostics }`, not debug backtraces. Source-local paths and bytes stay on this computer and are absent from public CI artifacts.

---

### Task 1: Independently testable chess game model

**Files:** Create `app/Cargo.toml`, `app/rust-toolchain.toml`, `app/.gitignore`, `app/LICENSE`, `app/THIRD_PARTY_NOTICES.md`, `app/crates/chess-core/Cargo.toml`, `app/crates/chess-core/src/{lib,model,position,error}.rs`, `app/crates/chess-core/tests/tree.rs`, and the two copied design documents.

**Interfaces:** Produces `Game::initial() -> Game`, `Game::from_fen(&str) -> Result<Game, CoreError>`, `Game::append_uci(NodeId, &str, &GameLimits) -> Result<NodeId, CoreError>`, `Game::position_at(NodeId) -> Result<PositionView, CoreError>`, `Game::san_at(NodeId) -> Result<Option<String>, CoreError>`, and `GameLimits::default()`. Consumes no project API.

- [x] Establish the isolated product repository and local Rust toolchain. Query the official stable channel, pin its actual version in `rust-toolchain.toml`, and record `rustc -V`, `cargo -V` and host target. Resolve published dependencies and inspect `cargo tree -d` for duplicate shakmaty versions. Include the GPL license text and dependency notices; retain the MIT reference notice when its code is ported in Task 5.
- [x] Add the following behavioural test and companion tests for a repeated move reusing its existing child, a new move becoming an ordered sibling, invalid FEN, null move while in check, and an invalid node ID.

```rust
#[test]
fn illegal_edit_keeps_tree_unchanged() {
    let mut g = chess_core::Game::initial();
    let limits = chess_core::GameLimits::default();
    let e4 = g.append_uci(0, "e2e4", &limits).unwrap();
    let before = g.nodes.len();
    assert!(g.append_uci(e4, "e7e4", &limits).is_err());
    assert_eq!(g.nodes.len(), before);
    assert_eq!(g.nodes[0].children, vec![e4]);
    assert_eq!(g.san_at(e4).unwrap().as_deref(), Some("e4"));
}
```

- [x] Run `cargo test -p chess-core --test tree` and establish the failure.
- [x] Implement arena-backed nodes with checked IDs, parent traversal and legal validation using shakmaty. Validate before mutating; reject invalid/cyclic parent relationships and over-limit nodes. Nested-variation depth is checked by the PGN/CBG readers, not by counting parent-chain plies. Parse UCI through `UciMove::to_move`; compute SAN against the pre-move position. Serialize FEN using `EnPassantMode::Always` consistently in the model and comparison normalization. Treat `0000` separately through a validated turn swap, rejecting it in check; do not label null moves ordinary legal moves. Avoid storing a full copied tree or board history on every node.

```rust
// Workspace manifest foundation; add later crates when their deliverables begin.
[workspace]
members = ["crates/chess-core"]
resolver = "3"
[workspace.package]
edition = "2024"
license = "GPL-3.0-or-later"
[workspace.dependencies]
shakmaty = "0.30"
serde = { version = "1", features = ["derive"] }
thiserror = "2"
```

- [x] Rerun the tree tests and `cargo fmt --all --check`; commit the model and repository foundation with `feat: add validated chess game model`.

### Task 2: PGN collections, variations, comments and export

**Files:** Create `app/crates/chess-core/src/pgn.rs`, `app/crates/chess-core/tests/pgn.rs`, `app/tests/fixtures/README.md`; modify `app/crates/chess-core/{Cargo.toml,src/lib.rs,src/model.rs}`.

**Interfaces:** Consumes Task 1 `Game`. Produces `parse_pgn(&[u8], &GameLimits) -> Result<Vec<Game>, CoreError>`, `write_pgn(&Game) -> Result<String, CoreError>`, `Game::set_comments(NodeId, Vec<String>) -> Result<(), CoreError>` and `Game::set_nags(NodeId, Vec<u16>) -> Result<(), CoreError>`. Collection caps are enforced in this parser in addition to per-game limits.

- [x] Add this tree round-trip test plus multigame input, black-to-move FEN, escaped tag quotes/backslashes, NAGs, comments before a first move, invalid SAN, unbalanced variation and collection-limit tests.

```rust
#[test]
fn pgn_preserves_ordered_alternatives_and_comments() {
    let bytes = b"[Event \"Synthetic\"]\n[Result \"*\"]\n\n1. e4 {centre} e5 (1... c5 $1) 2. Nf3 *";
    let limits = chess_core::GameLimits::default();
    let g = chess_core::parse_pgn(bytes, &limits).unwrap().remove(0);
    let e4 = g.nodes[0].children[0];
    assert_eq!(g.nodes[e4 as usize].comments, vec!["centre"]);
    let replies = &g.nodes[e4 as usize].children;
    assert_eq!(replies.len(), 2);
    assert_eq!(g.nodes[replies[0] as usize].uci.as_deref(), Some("e7e5"));
    assert_eq!(g.nodes[replies[1] as usize].uci.as_deref(), Some("c7c5"));
    let text = chess_core::write_pgn(&g).unwrap();
    let again = chess_core::parse_pgn(text.as_bytes(), &limits).unwrap().remove(0);
    assert_eq!(again, g); // derive structural equality on Game and Node
}
```

- [x] Run `cargo test -p chess-core --test pgn` and confirm failures.
- [x] Implement a pgn-reader visitor with an explicit variation stack. Variation start restores the position before the preceding move; variation end restores its saved continuation. Validate every SAN through the core; never accept pgn-reader syntax alone as legal chess. Normalize missing seven-roster tags on import and `Game::initial` (`?` names/event/site/round, `????.??.??` date, `*` result), so export/import structural equality is well-defined. Preserve root comments, arbitrary tag values and all ordered branches. Conflicting result tag/movetext produces a diagnostic error rather than silently choosing one. The writer emits the seven tag roster, custom tags, `SetUp`/`FEN` when needed, move numbers from position turn/fullmove, NAGs and balanced variations. Escape tag strings. Reject comment edits containing `{` or `}` with `unrepresentableComment`, retaining existing comments, rather than inventing a nonstandard escape or silently changing text.

```rust
#[test]
fn unrepresentable_comment_does_not_destroy_previous_comment() {
    let mut g = chess_core::Game::initial();
    g.set_comments(0, vec!["keep me".into()]).unwrap();
    assert!(g.set_comments(0, vec!["brace }".into()]).is_err());
    assert_eq!(g.nodes[0].comments, vec!["keep me"]);
}
```

- [x] Rerun all core tests, then commit with `feat: import and export editable PGN game trees`.

### Task 3: Bounded classic CBH headers and source reads

**Files:** Create `app/crates/cbh-reader/Cargo.toml`, `app/crates/cbh-reader/src/{lib,error,limits,source,header}.rs`, `app/crates/cbh-reader/tests/{headers.rs,support/mod.rs,support/family.rs}`; modify `app/Cargo.toml`.

**Interfaces:** Produces `ClassicHeader::parse(&[u8], u64) -> Result<ClassicHeader, CbhError>`, fields `entry_count: u64`; `parse_record(&[u8], u64) -> Result<RawEntry, CbhError>`; `SourceFile::open(&Path) -> Result<SourceFile, CbhError>` and `SourceFile::read_exact_range(u64, usize) -> Result<Vec<u8>, CbhError>`. `RawEntry` contains ordinal, kind, deleted flag, optional game-only IDs/offsets/date/result/ratings/ECO raw value. `CbhError` distinguishes invalidHeader, truncated, sourceChanged, bounds, unsupported and limitExceeded.

- [x] Add header/record tests using authored fixed offsets, not the production writer. Set `header[7..10]` to BE24 3 and a synthetic file length of 138; assert two data entries. Add sentinel `ff ff ff` with BE64 at 36, zero count, nonmultiple file size, unknown header, unknown result/date and type nibble 3 tests. A text record must not expose game player IDs.

```rust
#[test]
fn classic_count_excludes_header() {
    let mut h = [0u8; 46];
    h[..6].copy_from_slice(&[0, 0, 0x2c, 0, 0x2e, 1]);
    h[7..10].copy_from_slice(&[0, 0, 3]);
    let parsed = cbh_reader::header::ClassicHeader::parse(&h, 138).unwrap();
    assert_eq!(parsed.entry_count, 2);
    assert!(cbh_reader::header::ClassicHeader::parse(&h, 137).is_err());
}
```

- [x] Run `cargo test -p cbh-reader --test headers` and confirm failures.
- [x] Implement explicit BE24/BE32/BE64 helpers, checked arithmetic and native-corroborated count semantics, not the reference script's physical-record-count shortcut. A count below physical capacity may represent spare records: permit only complete 46-byte physical records, expose the header count, warn about trailing capacity, and reject counts above capacity. Ordinary ordinals start at 1; ordinal 0 is never a game. Recognize the two reference classic signatures and the corroborated extended-header path; unknown signatures fail as unsupported. The extended count is valid when header low nibble is zero, BE16 at bytes 1..3 is greater than `0x24`, and BE24 at bytes 7..10 equals `0xffffff`; then read BE64 at bytes 36..44. This condition is retained in native `old-cbh-header-conversion` evidence. Use file metadata and read-only handles; verify current length, modification time and path/file identity before and after each request, using the cross-platform `same-file` crate handles if needed. Read through a per-file lock and seek/read, or platform positional-read abstraction, with no mmap dependency. Implement bounded read metrics for tests.

```rust
// Before any allocation or seek:
let end = offset.checked_add(length as u64).ok_or(CbhError::Bounds)?;
if end > current_length { return Err(CbhError::Truncated); }
```

- [x] Add a test that opens a temporary source, truncates it, then requests a range and receives `sourceChanged`/`truncated` without panic. Run the header tests and commit with `feat: read classic CBH headers with bounded access`.

### Task 4: Companion discovery and cached entity names

**Files:** Create `app/crates/cbh-reader/src/{companions,entities}.rs`, `app/crates/cbh-reader/tests/entities.rs`; extend `app/crates/cbh-reader/tests/support/family.rs` and `app/crates/cbh-reader/src/lib.rs`.

**Interfaces:** Consumes Task 3 source reads. Produces `CompanionSet::discover(&Path) -> Result<CompanionSet, CbhError>`, `CompanionSet::get(&str) -> Option<&Path>`, `EntityTables::open(&CompanionSet, usize) -> Result<EntityTables, CbhError>`, `EntityTables::player(u32) -> Result<EntityText, CbhError>` and `EntityTables::tournament(u32) -> Result<TournamentText, CbhError>`. `EntityText` has `text: Option<String>` and diagnostics; `TournamentText` has event/site of that type. Missing tables yield diagnostics and unknown names while preserving IDs.

- [x] Add case-mixed basename/extension, missing CBG, duplicate `Demo.cbp`/`demo.CBP`, file path containing spaces, CPB/CBT 28/32-byte headers, invalid entity IDs, unterminated names, valid UTF-8 accents and invalid UTF-8 tests. Ambiguity uses pure candidate matching tests on case-insensitive hosts, since those hosts cannot create both filenames.

```rust
#[test]
fn duplicate_folded_companion_is_ambiguous() {
    let candidates = ["Demo.cbp", "demo.CBP"];
    assert!(cbh_reader::companions::choose_candidate(
        "demo", "cbp", &candidates
    ).is_err());
}
```

- [x] Run `cargo test -p cbh-reader --test entities` and confirm failures.
- [x] Implement single-directory discovery, exact-case basename preference only when it resolves unambiguously, and bounded reads of known entity records. Determine start as `28 + byte[24]` only for supported values 0/4; validate count/stride/file bounds. CBP stride 67 with surname `[9..39]`, given name `[39..59]`; CBT stride 99 with event `[9..49]`, site `[49..79]`. Decode validated UTF-8, reporting replacement/unresolved encoding for invalid bytes; do not assume a guessed codepage. Validate record-size header fields against known layouts. Cache normalized names in a byte-accounted LRU capped by the supplied budget; oversized entries bypass caching.

```rust
pub fn choose_candidate<'a>(
    stem: &str, extension: &str, candidates: &'a [&str]
) -> Result<Option<&'a str>, CbhError> {
    // Match basename+extension case-insensitively, then reject multiple matches.
    // The implementation must also handle actual Path/OsStr names without
    // lossy Unicode conversion of the selected filesystem path.
    let wanted = format!("{stem}.{extension}");
    let matches: Vec<_> = candidates.iter().copied()
        .filter(|name| name.eq_ignore_ascii_case(&wanted)).collect();
    match matches.as_slice() {
        [] => Ok(None), [one] => Ok(Some(*one)),
        _ => Err(CbhError::AmbiguousCompanion),
    }
}
```

- [x] Verify cache memory accounting through a sequence of distinct entity lookups; rerun tests and commit with `feat: resolve CBH companions and entity names`.

### Task 5: Standard classic CBG move decoding

**Files:** Create `app/crates/cbh-reader/src/moves/{mod,tables,pieces,decode}.rs`, `app/crates/cbh-reader/tests/moves.rs`, `app/crates/cbh-reader/tests/support/encoder.rs`; update notices and `app/crates/cbh-reader/src/lib.rs`.

**Interfaces:** Consumes Task 1 core. Produces `moves::decode_classic(&[u8], &ReadLimits) -> Result<Game, CbhError>`, reading a complete CBG record including its four-byte header. Internals: `PieceState` maps CB piece ordinals to squares; `TokenCursor` tracks byte position and wrapping processed-move count; no filesystem access in the decoder.

- [x] Add this independent literal-byte case, then tests for captures followed by another same-kind piece move, both castlings, en passant, a two-byte move, counter wrapping after 256 processed moves and truncated two-byte tokens.

```rust
#[test]
fn literal_initial_e4_e5_decodes() {
    // Header size 7; e-pawn two-square token ff, then ff+1 wraps to 00;
    // line terminator 0c + two processed moves = 0e.
    let record = [0, 0, 0, 7, 0xff, 0x00, 0x0e];
    let g = cbh_reader::moves::decode_classic(
        &record, &cbh_reader::ReadLimits::default()
    ).unwrap();
    let first = g.nodes[0].children[0];
    let second = g.nodes[first as usize].children[0];
    assert_eq!(g.nodes[first as usize].uci.as_deref(), Some("e2e4"));
    assert_eq!(g.nodes[second as usize].uci.as_deref(), Some("e7e5"));
    assert_eq!(g.nodes.len(), 3);
}
```

- [x] Run `cargo test -p cbh-reader --test moves` and confirm failures.
- [x] Port the MIT reference tables and relevant logic from pinned revision `42b3592738062db1f768239e85df1b98cb1cead9`, retaining attribution. Audit, rather than mechanically preserving, its ignored en-passant pawn state and permissive exception/partial-game behaviour. Initialize file-major piece ordinals from the reference's explicit starting lists. Resolve one-byte deltas, king castling tokens, two-byte source/destination/promotion, capture renumbering and promotion insertion; validate each resulting move with the core before committing state. Reconcile en-passant captures in both ordinal and chess states.

```rust
let token = stored.wrapping_sub(processed);
// Special codes 29/DC/0C/9F do not increment the one-byte counter.
// The 29 payload is deobfuscated with the current count and then advances it
// once. Ordinary tokens, including AA, advance the counter once.
```

- [x] Add a synthetic test encoder that starts from explicitly provided UCI moves and authored expected PGNs. Its expected results must not be obtained by decoding its own output. Include literal bytes above as an independent anchor. Reject unknown tokens, illegal moves and invalid source ordinals; do not return a partially valid game after failure. Run moves/core tests and commit with `feat: decode standard classic CBG games`.

### Task 6: Setup positions, nested branches and codec limits

**Files:** Create `app/crates/cbh-reader/src/moves/setup.rs`, `app/crates/cbh-reader/tests/setups.rs`; modify `app/crates/cbh-reader/src/moves/{decode,pieces}.rs` and synthetic encoder support.

**Interfaces:** Extends `decode_classic` without changing its signature. Produces internal `decode_setup(&[u8]) -> Result<(String, PieceState), CbhError>`. Error diagnostics identify flags, byte offsets and exhausted limits.

- [x] Add a setup with white king e1, black king e8 and white pawn a7 whose expected first move is `a7a8n`, plus a black-to-move setup, castling rights, en-passant setup, invalid/missing king and truncated 28-byte setup. Author occupied-square bit sequences independently from the decoder and document their file-major order.

```rust
#[test]
fn unsupported_flags_are_not_empty_games() {
    // Special-encoding bit 26 set, with a syntactically sized five-byte record.
    let record = [0x04, 0, 0, 5, 0x0c];
    let err = cbh_reader::moves::decode_classic(
        &record, &cbh_reader::ReadLimits::default()
    ).unwrap_err();
    assert_eq!(err.code(), "unsupportedEncoding");
}
```

- [x] Add nested/sibling variations whose independently authored expected PGN is `1. e4 e5 (1... c5 2. Nf3 (2. Nc3)) 2. Nf3 *`, plus a null move, filler token, unmatched branch end, missing root terminator and depth/node/byte-limit cases. Keep a literal hand-annotated branch token case alongside generated longer fixtures.
- [x] Run `cargo test -p cbh-reader --test setups` and confirm failures.
- [x] Decode the 28-byte setup: control bytes then 24-byte MSB-first occupancy/piece bitstream; empty square consumes one bit and occupied square five. Restore turn, castling, en-passant and move number from the audited reference, validate a standard position and map ordinals. The en-passant “absent” condition needs corroboration from reference/native evidence before accepting a setup; if unresolved for a particular flag combination return `unsupportedSetup` rather than fabricating rights. Branch frames clone only position/ordinal state and node ID; the byte move counter follows the audited global stream rule rather than resetting on branch restore. Require a terminal root `0c` and no unmatched frames; bound stack and arena growth before pushes. Mark unknown flag combinations unsupported, using `0x80000000` unencoded, `0x04000000` special, reference Chess960 mask `0x0a000000`, and `0x40000000` setup flags with independently validated combinations.
- [x] Run all CBH/core tests; record exactly which setup flags are supported in `app/docs/compatibility.md`; commit with `feat: support bounded CBH setup and variation decoding`.

### Task 7: Complete read-only database adapter and real-file inspection

**Files:** Extend `app/crates/cbh-reader/src/lib.rs`; create `app/crates/cbh-reader/src/database.rs`, `app/crates/cbh-reader/tests/database.rs`, `app/crates/cbh-reader/examples/inspect.rs`, `app/scripts/compare-corpus.py`, `app/docs/compatibility.md`.

**Interfaces:** Consumes Tasks 3-6. Produces `ClassicDatabase::open(&Path, ReadLimits) -> Result<ClassicDatabase, CbhError>`, `info(&self) -> &DatabaseInfo`, `page(&mut self, u64, usize) -> Result<EntryPage, CbhError>`, `load(&mut self, u64) -> Result<LoadedEntry, CbhError>`. Missing/unsupported entry content returns a `LoadedEntry` with no game and a status/diagnostic; catastrophic database/header I/O failures return `Err`.

- [x] Add end-to-end generated-family tests. A valid family yields named metadata and the literal e4/e5 tree; a missing CBG still lists metadata, but `load` has no game. A corrupt selected stream affects that entry only. A text entry has no player IDs interpreted as names. Deletion is a visible flag, not an implicit skip. Page overflow, zero limit, final page and ordinal out of range are explicit.

```rust
#[test]
fn cbh_without_moves_remains_browsable() {
    let family = support::family::minimal_family(false); // returns TempDir
    let mut db = cbh_reader::ClassicDatabase::open(
        &family.path().join("Synthetic.cbh"), cbh_reader::ReadLimits::default()
    ).unwrap();
    assert_eq!(db.page(1, 200).unwrap().entries.len(), 1);
    let loaded = db.load(1).unwrap();
    assert!(loaded.game.is_none());
    assert!(matches!(loaded.moves_status, cbh_reader::DataStatus::Missing));
}
```

- [x] Run `cargo test -p cbh-reader --test database` and confirm failures.
- [x] Implement the adapter using checked header offsets and bounded companion access. Map known result/date/Elo fields from native-corroborated records. Decode ECO only after verifying the packed-value formula against retained observations and independent synthetic boundary cases; an unverified value stays unknown with a diagnostic. Keep source/annotator IDs if names cannot be resolved; optional missing companions do not prevent moves. An annotation offset/file yields `NotDecoded`, never `Available`. Capture source identity changes between page/load requests.
- [x] Implement `inspect` with `--path`, `--page-start`, `--page-limit`, `--entry`, `--pgn-out` and `--json-out` arguments, rejecting unsupported switches and keeping errors separate from JSON output. `--pgn-out` exports only a supported selected game. Do not write into the source database directory by default.

```bash
cargo run -p cbh-reader --example inspect -- \
  --path '<database-path>' \
  --page-start 1 --page-limit 50 --entry 14 \
  --json-out ../analysis/workbench-local-entry14.json
```

- [x] Implement the optional comparison script with explicit path/reference arguments. Compare canonical ordered UCI trees, initial FEN, date and result for the 36 researched ordinals, then 200 deterministic spread ordinals; classify unsupported records separately from failures. Use the pinned reference with local Python chess only for this private development check. Store reports under parent `analysis/`, not `app/` or CI; do not commit games or bytes. Verify total bytes read, peak memory, first-page and selected-load durations. Add a synthetic sparse large-header-file test proving page reads do not scan the corpus.
- [x] Run all reader/core tests and the bounded real-file check. Failures prevent advertising general compatibility; document supported/unsupported counts. Commit with `feat: browse and export classic CBH databases`.

### Task 8: Desktop shell and typed document commands

**Files:** Create `app/package.json`, `app/package-lock.json`, `app/tsconfig*.json`, `app/vite.config.ts`, `app/index.html`, `app/src/main.tsx`, `app/src/App.tsx`, `app/src/api/{types,client}.ts`, `app/src-tauri/{Cargo.toml,build.rs,tauri.conf.json,capabilities/main.json}`, `app/src-tauri/src/{main,lib,commands,registry,documents}.rs`, `app/src-tauri/tests/commands.rs`; extend the workspace manifest and lockfile.

**Interfaces:** Consumes `ClassicDatabase` and core types. Produces registry service methods with these exact names, wrapped by same-named Tauri commands: `open_database(path: PathBuf, request_id: String) -> Result<OpenDatabaseResponse, AppError>`, `read_page(database_id: String, start: String, limit: usize, request_id: String) -> Result<PageResponse, AppError>`, `load_entry(database_id: String, ordinal: String, request_id: String) -> Result<GameResponse, AppError>`, `position_at(document_id: String, node_id: NodeId, request_id: String) -> Result<PositionResponse, AppError>`, `close_database(database_id: String) -> Result<(), AppError>`, and `close_document(document_id: String) -> Result<(), AppError>`. Each also receives managed application state in its Tauri wrapper. `AppError` serializes the shared wire error shape.

```typescript
// src/api/types.ts: owning DTO contract; mirror with serde Rust DTOs.
export type NodeId = number;
export type Status =
  "available" | "missing" | "unsupported" | "corrupt" | "notDecoded";
export interface Diagnostic {
  code: string;
  message: string;
  file?: string;
  offset?: string;
}
export interface EntrySummary {
  ordinal: string;
  kind: "game" | "text" | "unknown";
  deleted: boolean;
  white?: string;
  black?: string;
  event?: string;
  site?: string;
  date?: string;
  result?: string;
  whiteElo?: number;
  blackElo?: number;
  eco?: string;
  diagnostics: Diagnostic[];
}
export interface GameNode {
  id: NodeId;
  parent?: NodeId;
  uci?: string;
  san?: string;
  children: NodeId[];
  comments: string[];
  nags: number[];
}
export interface GameView {
  tags: Record<string, string>;
  startFen: string;
  nodes: GameNode[];
}
export interface OpenDatabaseResponse {
  requestId: string;
  databaseId: string;
  entryCount: string;
  companions: { extension: string; status: Status }[];
  diagnostics: Diagnostic[];
}
export interface PageResponse {
  requestId: string;
  entries: EntrySummary[];
  next?: string;
}
export interface GameResponse {
  requestId: string;
  documentId?: string;
  game?: GameView;
  summary: EntrySummary;
  metadataStatus: Status;
  movesStatus: Status;
  annotationsStatus: Status;
  diagnostics: Diagnostic[];
}
export interface PositionResponse {
  requestId: string;
  documentId: string;
  nodeId: NodeId;
  fen: string;
  turn: "white" | "black";
  legalUci: string[];
}
export interface WireError {
  code: string;
  message: string;
  diagnostics: Diagnostic[];
}
```

- [x] Resolve and lock the Tauri 2/React/Vite/TypeScript stack and Vitest/jsdom/testing-library test dependencies. Add scripts `dev`, `build`, `test`, `typecheck`, `tauri` and `check:distribution`. Keep Node's supported LTS version pinned for CI rather than assuming the locally installed Node 26 is the deployment requirement. Add Tauri dialog support and only the capabilities needed by the main window; no frontend filesystem/shell plugin. Configure a local-content CSP and disable arbitrary remote content navigation.
- [x] Add Rust registry tests for unknown handles, closed handles, two concurrently opened databases, decimal ordinals greater than JavaScript's safe-integer range, malformed ordinal strings and DTO serialization.

```rust
#[test]
fn diagnostic_offsets_are_decimal_strings_on_wire() {
    let d = crate::commands::DiagnosticDto::from(cbh_reader::Diagnostic {
        code: "bounds".into(), message: "outside file".into(),
        file: None, offset: Some(9_007_199_254_740_993),
    });
    let json = serde_json::to_value(d).unwrap();
    assert_eq!(json["offset"], "9007199254740993");
}
```

- [x] Run `cargo test -p openfile --test commands` and confirm failures. Test services independently of launching a webview.
- [x] Implement opaque handle registries with per-database locking and bounded loaded-document ownership. Use Tauri `spawn_blocking` for decoding; release the global registry lock before file work. A supported load creates an independent document handle; an unsupported load creates no editable document. Closing a database releases its files but does not invalidate an already loaded working copy. Default to four open databases and ten loaded documents; explicitly reject excess handles and close discarded/stale loads to prevent leaks. Backend DTO conversion computes SAN by a single traversal with incrementally maintained positions, avoiding per-node root replay and quadratic tree conversion.

```typescript
// src/api/client.ts: inject a typed facade into components for testability.
import { invoke } from "@tauri-apps/api/core";
export const readPage = (
  databaseId: string,
  start: string,
  limit: number,
  requestId: string,
) => invoke<PageResponse>("read_page", { databaseId, start, limit, requestId });
```

- [x] Render a minimal mounted desktop shell that shows backend open/page errors and a selected game count. Verify `npm run typecheck`, `npm run build`, Rust command tests and `npm run tauri -- build --no-bundle`. Launch `npm run tauri -- dev` locally and confirm the window mounts and reaches real Rust commands. Commit with `feat: connect desktop shell to chess database services`.

### Task 9: Database table, board and move-tree replay

**Files:** Create `app/src/state/{workbench,selection}.ts`, `app/src/components/{DatabaseTable,Board,NotationTree,Toolbar,Diagnostics}.tsx`, `app/src/styles/{layout,board}.css`, `app/src/tests/{setup.ts,harness.tsx,browse.test.tsx,selection.test.tsx}`; extend `app/src/App.tsx`, `app/src/api/client.ts` and Vitest configuration.

**Interfaces:** Consumes Task 8 API/DTOs. Produces injectable `WorkbenchApi` with `openDatabase`, `readPage`, `loadEntry`, `positionAt`, `closeDatabase`, `closeDocument` using the same request/result types. Produces `SelectionGate.begin() -> string`, `SelectionGate.accept(requestId: string) -> boolean`; `Board` receives FEN, flipped flag and legal UCI moves; `NotationTree` receives nodes, selection and `onSelect(NodeId)`. `App` receives optional `api` and `initialState` props, defaulting to the native API and clean state. Define `renderWorkbench({dirty?: boolean})` in `src/tests/harness.tsx`: use Testing Library `render` on App with mocked API and a synthetic selected document, add `userEvent.setup()` as `user`, and return the render result plus `api` and `user`. Task 10 extends this harness when adding edit/save methods.

- [x] Add an injected-API workflow test opening a generated-family DTO, paging, loading e4/e5, selecting e4, visiting a sibling, stepping back and flipping. Test that deleted/text rows are labelled, missing CBG explains replay unavailability and a failed selection leaves the existing working game visible. Add keyboard/focus tests for first/last/previous/next move; typing in text inputs must not trigger board shortcuts.

```typescript
it("rejects an older selection result", () => {
  const gate = new SelectionGate();
  const old = gate.begin();
  const current = gate.begin();
  expect(gate.accept(old)).toBe(false);
  expect(gate.accept(current)).toBe(true);
});
```

- [x] Run `npm test -- src/tests/browse.test.tsx src/tests/selection.test.tsx` and confirm failures.
- [x] Implement native open-file selection, a bounded 200-row table, next/previous pages and explicit decimal ordinal entry using `BigInt` only for arithmetic. Use persisted successful selection plus a separate loading target; request IDs guard database, page, game and position operations. Dispose stale successful document handles without changing displayed state. Render an original CSS/Unicode chess board initially, with square labels, high-contrast selection/focus and a promotion picker reserved for Task 10; copy no proprietary piece graphics. The notation view renders the main line and expanded branches in bounded windows with selection scrolled into view; it does not mount 100,000 DOM nodes. Use stable node IDs and show comments/NAGs available in the model. Make analysis pane space resizable in the layout, with pointer and keyboard resizing. Navigation actions always request the selected backend position.

```typescript
const requestId = gate.begin();
const result = await api.loadEntry(databaseId, ordinal, requestId);
if (!gate.accept(result.requestId)) {
  if (result.documentId) await api.closeDocument(result.documentId);
  return;
}
// Only a current successful result can replace the displayed document.
```

- [x] Run tests/typecheck, then open the real classic database in the desktop app. Confirm the first page, entry 14 variation tree, flip/navigation and readable diagnostics. Capture an original app screenshot and record timings in private validation notes. Commit with `feat: browse and replay CBH games in the desktop UI`.

### Task 10: Editable working copies, PGN opening and reliable saves

**Files:** Create `app/src-tauri/src/save.rs`, `app/src-tauri/tests/save.rs`, `app/src/state/editing.ts`, `app/src/components/{CommentEditor,UnsavedDialog}.tsx`, `app/src/tests/edit.test.tsx`; modify `app/src-tauri/src/{documents,commands}.rs`, `app/src/api/{client,types}.ts`, board/toolbar/App and core tests as needed.

**Interfaces:** Consumes core PGN and existing document registry. Produces backend `open_pgn(path: PathBuf, request_id: String) -> Result<PgnCollectionResponse, AppError>`, `load_pgn_game(collection_id: String, index: usize, request_id: String) -> Result<DocumentResponse, AppError>`, `close_pgn(collection_id: String) -> Result<(), AppError>`, `edit_document(document_id: String, edit: Edit, expected_revision: String) -> Result<DocumentResponse, AppError>`, `save_document(document_id: String, destination: PathBuf, expected_revision: String, allow_incomplete: bool) -> Result<SaveResponse, AppError>`. Wire revisions are decimal strings, parsed to internal u64 in the service before validation. `Edit` is tagged `appendMove {parent, uci}`, `setComments {node, comments}`, or `setNags {node, nags}`. `DocumentResponse` has documentId, requestId when applicable, game, revision and dirty. `PgnCollectionResponse` has requestId, collectionId, gameCount and per-game tag summaries. `SaveResponse` has savedRevision and destination. `save::atomic_pgn(&Path, &str) -> Result<(), AppError>` is independently testable.

- [x] Add tests for legal click-to-move, promotion choice, branching from an earlier move, comment edit round trips, two-game PGN selection and stale edit revision. Changing database/entry, closing a document/window or opening PGN while dirty must run save/discard/cancel. Cancel leaves selection, document and engine position unchanged. Save cancellation/failure keeps dirty state. Read/parse failure keeps the current working game.

```typescript
it("keeps an edited game when replacement is cancelled", async () => {
  // renderWorkbench is the test harness from Task 9 with injectable DTOs/API.
  const ui = renderWorkbench({ dirty: true });
  await ui.user.click(ui.getByRole("button", { name: "Open database" }));
  await ui.user.click(ui.getByRole("button", { name: "Cancel" }));
  expect(ui.api.openDatabase).not.toHaveBeenCalled();
  expect(ui.getByText("Unsaved changes")).toBeVisible();
});
```

- [x] Run `npm test -- src/tests/edit.test.tsx` and `cargo test -p openfile --test save`; confirm behavioural failures.
- [x] Implement revisioned backend edits with mutation only after core validation. An appended existing move only selects it; actual content changes increment revision. Load capped PGN collections using the core parser, applying aggregate limits and closing unused collection handles. UI sends exact legal UCI moves from selected-square pairs; promotion candidates require explicit queen/rook/bishop/knight selection. Add a visible compatibility summary before exporting a CBH-derived document whose original annotations or other content are unavailable; `allowIncomplete` records this acknowledgement. The backend also checks it, so direct command invocation cannot silently omit undecoded content.
- [x] Write UTF-8 PGN to a `NamedTempFile` in the destination directory, flush/sync, then persist/replace without deleting the old file first. Preserve permissions where supported and surface platform-specific replacement errors. On success mark clean only if the current revision equals `savedRevision`; an edit during save remains dirty. Refuse a destination that resolves to any source companion or has a CBH-family extension. Overwrite confirmation happens through the native save dialog/explicit existing-target prompt. If an additional atomic replacement primitive is needed on Windows, choose and verify it in the save module without weakening the contract.

```rust
#[test]
fn failed_replacement_preserves_existing_destination() {
    // Force the atomic-write implementation's pre-persist failure through an
    // injected writer hook; no permissions-based assumptions on CI/root users.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keep.pgn");
    std::fs::write(&path, "old contents").unwrap();
    let err = crate::save::atomic_pgn_with_writer(&path, |writer| {
        std::io::Write::write_all(writer, b"partial")?;
        Err(std::io::Error::other("simulated disk failure"))
    });
    assert!(err.is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old contents");
}
```

- [x] Define `atomic_pgn_with_writer(&Path, impl FnOnce(&mut File) -> io::Result<()>)` as an internal/testable save primitive used by `atomic_pgn`; include target-source alias and revision-race tests. Run all affected suites and a native save/reopen round trip with a synthetic and real decoded game. Commit with `feat: edit chess games and save PGN safely`.

### Task 11: Typed UCI responses and configurable options

**Files:** Create `app/crates/engine/{Cargo.toml,src/lib.rs,src/protocol.rs,src/options.rs,src/error.rs,tests/protocol.rs}`; add the crate to the workspace.

**Interfaces:** Produces `parse_line(&str) -> Result<Option<UciResponse>, EngineError>`, `EngineOption {name, kind, default, min, max, vars}`, `validate_option(&EngineOption, &str) -> Result<String, EngineError>` and these response structures.

```rust
pub enum UciResponse {
    Id { field: String, value: String }, Option(EngineOption),
    UciOk, ReadyOk, Info(AnalysisInfo),
    BestMove { best: String, ponder: Option<String> },
}
pub enum Score {
    Cp { value: i32, bound: Option<String> },
    Mate { moves: i32, bound: Option<String> },
}
pub struct AnalysisInfo {
    pub depth: Option<u32>, pub nodes: Option<u64>,
    pub multipv: u32, pub score: Option<Score>, pub pv: Vec<String>,
}
pub enum OptionKind { Check, Spin, Combo, String, Button }
// EngineOption.default/min/max are Option<String>/Option<i64>/Option<i64>;
// vars is Vec<String>. Preserve spaces in option names and values.
```

- [x] Add parser tests for reordered `info` fields, score bounds, negative mate, MultiPV, unknown fields, `info string`, missing numeric values, large counters and option names with spaces. Add option validation tests for check/spin/combo/string/button and reject embedded CR/LF in any sent value.

```rust
#[test]
fn score_bound_and_multipv_are_preserved() {
    let line = "info depth 18 multipv 2 score cp -31 lowerbound nodes 9007199254740993 pv e2e4 e7e5";
    let parsed = engine::parse_line(line).unwrap().unwrap();
    let engine::UciResponse::Info(info) = parsed else { panic!("expected info") };
    assert_eq!(info.multipv, 2);
    assert_eq!(info.nodes, Some(9_007_199_254_740_993));
    assert!(matches!(info.score, Some(engine::Score::Cp { value: -31, .. })));
}
```

- [x] Run `cargo test -p engine --test protocol` and confirm failures.
- [x] Implement a token cursor that skips unsupported fields without consuming subsequent known ones incorrectly; treat the rest after `info string` as text. Malformed known fields are diagnostic errors, not process crashes. Parse option records using `name`/`type` and type-specific fields while retaining multiword values. Reject a line over 16 KiB before parsing. Validate UCI position strings from trusted core state, never concatenate editable free text directly into commands. Score perspective is the root side to move; conversion belongs in Task 13, not the parser.
- [x] Run protocol tests and commit with `feat: parse UCI analysis and engine options`.

### Task 12: UCI child-process lifecycle and position switching

**Files:** Create `app/crates/engine/src/session.rs`, `app/crates/engine/src/bin/mock-uci.rs`, `app/crates/engine/tests/lifecycle.rs`; modify crate exports and manifest.

**Interfaces:** Consumes Task 11 protocol and Task 1 positions. Produces `EngineSession::spawn(EngineSpec, Duration) -> Result<EngineSession, EngineError>`, `set_option(&mut self, &str, &str) -> Result<(), EngineError>`, `analyse(&mut self, AnalysisRequest) -> Result<(), EngineError>`, `stop(&mut self) -> Result<(), EngineError>`, `shutdown(&mut self) -> Result<(), EngineError>`, `recv_event(&mut self, Duration) -> Result<Option<EngineEvent>, EngineError>`. `EngineSpec` has executable PathBuf and argument Vec<OsString>; `AnalysisRequest` has request_id string, validated start_fen string, history Vec<String>, root_turn string. `EngineEvent` has request_id string and tagged payload info/bestmove/error/status. Sessions own and reap their process/reader tasks.

- [x] Add integration tests invoking Cargo's built mock-uci binary, including `uci` → `uciok`, `isready` → `readyok`, options, process death, malformed lines, stderr saturation, ignored stop, handshake timeout and ignored quit. Copy the mock binary to a temporary directory with spaces to test executable paths. Never require Python or a downloaded engine for public tests.

```rust
#[test]
fn delayed_old_info_is_not_assigned_to_new_position() {
    let mut s = support::mock_session("late-info-before-bestmove");
    s.analyse(support::request("old", "e2e4")).unwrap();
    s.analyse(support::request("new", "d2d4")).unwrap();
    let events = support::events_until_current_info(&mut s, "new");
    assert!(events.iter().filter(|e| e.request_id == "new")
        .all(|e| !support::contains_old_marker(e)));
    s.shutdown().unwrap();
}
```

- [x] Implement the test helpers in `app/crates/engine/tests/support/mod.rs` with exact request construction and bounded deadlines; run lifecycle tests and confirm failures.
- [x] Use `std::process::Command` with piped streams and structured arguments; drain stdout/stderr concurrently with bounded buffers. Handshake deadline defaults to 5 seconds. New analysis sends `stop` and waits for the previous search's `bestmove` completion boundary before changing the active request ID and sending the new `position`/`go infinite`. UCI lines do not carry IDs: simply stamping late lines with the newest ID is incorrect. `isready` is not a substitute for search completion. If stop misses its 2-second deadline, terminate/reap/restart with handshake and cached validated options before the new search. Coalesce `info` updates by request/MultiPV and preserve completion/errors even when the queue is full.

```text
Idle -> Handshaking -> Ready -> Searching(request A)
Searching(A) -> Stopping(A) -> bestmove(A) -> Ready -> Searching(B)
Stopping timeout -> kill/reap -> handshake -> apply options -> Searching(B)
Any state -> Shutdown: stop, quit, 1-second wait, then kill/reap if required
```

- [x] Apply options only when ready, then wait for `readyok` before search. Synchronize transitions in one session owner to prevent conflicting writes. Closing a document stops its search; app shutdown shuts down all sessions and joins reader workers. Validate every PV move when rendering but preserve raw diagnostic information for invalid engine output. Run lifecycle/protocol tests and commit with `feat: manage cancellable local UCI engine sessions`.

### Task 13: Engine selection, options and live analysis pane

**Files:** Create `app/src-tauri/src/engine_commands.rs`, `app/src/state/analysis.ts`, `app/src/components/{EnginePanel,EngineOptions}.tsx`, `app/src/tests/engine.test.tsx`; modify backend registry/lib, API contracts/client and App.

**Interfaces:** Consumes Task 12 sessions and document registry. Produces commands `choose_engine`, `set_engine_option`, `start_analysis`, `stop_analysis`, `close_engine` through a managed single active session. `choose_engine` receives a user-selected executable path and returns opaque engineId, identity/options and status. `start_analysis(engineId, documentId, nodeId, requestId)` derives validated FEN/history/turn from the backend document, not frontend free text. A Tauri channel sends `EngineEventDto {requestId, payload}`; serialize u64 nodes as decimal strings. `AnalysisGate` records the active request/node and filters channel events.

- [x] Add UI tests for cancelled engine selection, missing executable, option form validation, start/stop, position change, black-to-move score conversion, bounded scores, mate display, stale request suppression and process-error recovery. Use an injected channel adapter so ordinary component tests need no desktop engine process.

```typescript
it("shows a black-root cp score from White’s perspective", () => {
  expect(formatScore({ kind: "cp", value: -31 }, "black", "white")).toBe(
    "+0.31",
  );
  expect(formatScore({ kind: "mate", moves: 3 }, "black", "white")).toBe("-M3");
});
```

- [x] Define `formatScore(score: ScoreDto, rootTurn: Color, perspective: Color): string`, `Color = 'white' | 'black'` and `ScoreDto` as the tagged Task 11 score DTO. Test lower/upper bound inversion when perspective is flipped. Run `npm test -- src/tests/engine.test.tsx` and confirm failures.
- [x] Implement an executable picker and supported-option controls. Display root position/request status and label “Score from White's perspective”; preserve root side internally. Convert PV moves to SAN from their exact analysed position and stop conversion at an invalid move with a diagnostic. Show depth, nodes, score, MultiPV and transient engine failures without replacing the working game. Coalesce visual updates at roughly 125 ms; retain the latest line per MultiPV. Node selection starts a fresh request only when analysis is enabled. Clean up channel subscriptions and engine handles on close; stale UI events are filtered in addition to the backend completion barrier.
- [x] Find an already installed compatible engine for the manual check. If none exists, obtain an official open-source engine build through the normal tool/network approval mechanism, keep it in local tooling and record provenance; do not bundle it. Analyse a synthetic starting position and a real decoded game, change node rapidly, stop and close the window, then confirm no child remains. Rerun UI/engine suites and commit with `feat: analyse selected positions with local UCI engines`.

### Task 14: Native builds, distribution boundary and acceptance evidence

**Files:** Create `app/.github/workflows/desktop.yml`, `app/scripts/check-distribution.mjs`, `app/docs/validation.md`, `app/README.md`; update compatibility/notices and manifests as required by verified builds.

**Interfaces:** Consumes all tasks. Produces reproducible build/test commands, a source inclusion check, native build jobs and an evidence-backed compatibility statement. No hosted repository is assumed.

- [x] Add distribution-check tests using a temporary candidate list: proprietary extensions such as `.cbh/.cbg/.cba/.2cbh`, extracted Windows binaries, parent paths, commercial PGN sample names and local absolute source paths are rejected; authored code, notices and documented synthetic fixture PGNs are accepted. Runtime launch/build binaries are generated outputs, not source-package inputs. The checker uses an explicit source allowlist and checks tracked files, not a blacklist alone.

```javascript
import { describe, expect, it } from "vitest";
import { allowedSourcePath } from "../scripts/check-distribution.mjs";
describe("source distribution boundary", () => {
  it("rejects research/database material", () => {
    expect(allowedSourcePath("../analysis/classic-sample-games.pgn")).toBe(
      false,
    );
    expect(allowedSourcePath("fixtures/Mega Database 2025.cbh")).toBe(false);
    expect(allowedSourcePath("crates/cbh-reader/src/header.rs")).toBe(true);
  });
});
```

- [x] Run `npm test -- tests/distribution.test.ts` and establish failure; implement `allowedSourcePath(path: string): boolean` plus the tracked-file/dependency-notice audit. Create `app/tests/distribution.test.ts` and include it in Vitest configuration.
- [x] Add CI jobs for `windows-latest`, `macos-latest` and `ubuntu-24.04`, each checking out only product source, installing the pinned Rust toolchain and supported Node LTS, using `npm ci`, and running locked tests/builds. Linux installs the official Tauri GTK/WebKit build prerequisites. Run `cargo test --workspace --locked`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `npm test`, `npm run typecheck`, `npm run build`, `npm run check:distribution` and `npm run tauri -- build --no-bundle`. Preserve command outputs and host/target/version metadata. Use project-relative binary paths under the workspace `target/`, not an assumed standalone `src-tauri/target/`.

```yaml
# Job skeleton; pin maintained action revisions when implementing the workflow.
strategy:
  fail-fast: false
  matrix:
    os: [windows-latest, macos-latest, ubuntu-24.04]
runs-on: ${{ matrix.os }}
defaults:
  run:
    working-directory: .
```

- [x] Exercise the actual macOS desktop with a native file picker, real database, branch navigation, keyboard focus, comment edit, save/discard/cancel, reopened PGN and real engine. Automated frontend tests cover the full workflow against an injected typed backend; Rust integration tests cover actual file/parser/save/process behaviour. Record that distinction rather than claiming mocked tests prove native integration. If adding native WebDriver automation, use the current official cross-platform embedded-driver guidance and exclude its test plugins/server from release builds.
- [x] Run the larger bounded corpus comparison and performance/memory checks from Task 7 after final changes. Summarize counts and all unsupported modes, annotation omissions, OS targets, timings and unexecuted checks in validation/compatibility docs. Do not commit proprietary game exports or local source paths to product documentation; use `<database-path>` examples there. Include quickstart commands, companion requirements, GPL/dependency notices and keyboard actions in README.
- [x] Windows/Linux evidence must come from actual native runners or an already authorized local VM/host. If those runners are unavailable, finish the local runnable app and committed CI configuration, report Windows/Linux verification as outstanding, and keep overall first-release acceptance open. Do not publish a repository or use the user's GitHub account solely to manufacture CI access without explicit publication authorization.
- [ ] Obtain the execution method's required fresh final review, resolve material defects with targeted red/green tests, rerun affected checks and record results. Commit with `build: verify desktop targets and document CBH compatibility`. Show the user the local app, commands, source path and exact remaining compatibility gaps.

## Milestones and completion criteria

| Milestone                  | Tasks | Demonstrable result                                                                           |
| -------------------------- | ----- | --------------------------------------------------------------------------------------------- |
| Format foundation          | 1-7   | Inspect real CBH headers and export supported games with correct ordered move trees           |
| Usable desktop             | 8-10  | Open a real CBH family, replay/edit a working game and save/reopen PGN                        |
| Analysis workbench         | 11-13 | Configure a local UCI engine and analyse the selected position reliably                       |
| Verified first deliverable | 14    | Actual native builds on all three OS families and evidence-backed compatibility documentation |

Do not stop at an earlier milestone and describe the approved first deliverable as complete. Conversely, do not hold back a working local app solely because external native runners are unavailable: deliver it with those checks explicitly outstanding. Full ChessBase feature equivalence remains the later stages in the approved spec.

## Plan self-review and requirement mapping

| Approved requirement                                                               | Implementation and evidence |
| ---------------------------------------------------------------------------------- | --------------------------- |
| CBH family discovery, partial-open status and paged metadata                       | Tasks 3/4/7/8/9             |
| Standard moves, setup positions, ordered variations and explicit unsupported modes | Tasks 1/5/6/7               |
| Legal board replay and keyboard/flip/navigation                                    | Tasks 1/8/9                 |
| Working-copy editing, comments/NAGs, PGN import/export and dirty-state handling    | Tasks 2/10                  |
| Read-only source, limits, cache budget, asynchronous decoding and stale selection  | Tasks 3/4/6/7/8/9/10        |
| Engine options, handshake, scores/MultiPV, stop/restart and cleanup                | Tasks 11/12/13              |
| Independent fixtures, local corpus checks and platform verification                | Tasks 1-7/10-14             |
| Open-source distribution boundary, notices and compatibility claims                | Tasks 1/5/7/14              |

Reviewed for spec coverage, named-type consistency, limit behaviour, fixture independence and all five Review Focus classes. No approved first-stage feature is intentionally omitted. Dependency patch versions and platform build results are recorded at execution time from actual resolution/runs, rather than asserted by this plan.

## Primary references used for planning

- [Pinned MIT decoder](https://github.com/asdfjkl/cbh2pgn/tree/42b3592738062db1f768239e85df1b98cb1cead9): code-port provenance; local source at `tooling/cbh2pgn-reference/`.
- [shakmaty UCI move API](https://docs.rs/shakmaty/latest/shakmaty/uci/enum.UciMove.html): legal move conversion and standard castling representation.
- [pgn-reader API](https://docs.rs/pgn-reader/latest/pgn_reader/): streaming visitor; SAN legality must be validated separately.
- [Tauri commands](https://v2.tauri.app/develop/calling-rust/) and [frontend channels](https://v2.tauri.app/develop/calling-frontend/): command/stream boundaries.
- [Tauri dialog plugin](https://v2.tauri.app/plugin/dialog/) and [platform prerequisites](https://v2.tauri.app/start/prerequisites/): file selection and native builds.
- [Tauri WebDriver guidance](https://v2.tauri.app/develop/tests/webdriver/): optional native automation; renderer mocks remain a separate verification layer.
- [Local research](compatibility.md) and [bounded verification](validation.md): corroborated layouts and current compatibility limits.

Status: Approved by the user; native execution selected on 2026-10-09. Tasks 1–13 are implemented and committed. Task 14 local packaging/evidence is implemented; final review and its verified fixes are the remaining execution step. Windows/Linux native acceptance remains open under the explicit unavailable-runner path above. See validation.md for current evidence.
