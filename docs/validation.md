# Validation evidence

Status on 2026-10-11: a usable native macOS development app is available.
Overall first-release acceptance remains open until Windows and Linux native
builds and desktop workflows pass. No hosted CI run or cross-platform release
has been claimed.

## Host and automated checks

The local host is macOS/Darwin 25.6.0, arm64, with Xcode. Rust 1.99.0,
Node 26.5.1 and npm 11.17.0 were used locally; CI selects Node 24 LTS.
Cargo and npm use committed lockfiles. Native build paths use workspace
`target/`, not `src-tauri/target/`.

Before final review, 86 Rust integration tests and 34 frontend/source tests
pass. Workspace rustfmt and strict Clippy, TypeScript and the production
frontend build pass. The final rebuilt macOS app bundle includes LICENSE, third-party notices and
all dependency notice resources. Final review is recorded below after its fix pass.
Rust tests exercise legal move/PGN trees, bounded CBH-family reads, entity
tables, setup/variation decoding, document revisions/atomic saves and actual
mock UCI child-process lifecycle. Frontend tests use an injected typed backend
to exercise navigation, edits, dirty guards, stale selections and analysis.
These frontend mocks do not establish native platform behavior.

## Native desktop checks

The macOS app was opened and exercised through its actual WebView and native
file dialogs:

- Opened an 11,151,119-entry classic CBH family; browsed paged metadata and
  replayed supported games, including an entry with 66 decoded moves.
- Navigated ordered branches, Previous/Next, keyboard focus and board flip.
- Opened an authored two-game PGN, added a legal variation/comment, saved through
  the native dialog and reopened it with the variation/comment intact.
- Exported a real supported CBH game after acknowledging omitted original
  annotations; independent python-chess comparison matched all 66 moves,
  ordered branches and White/Black/Date metadata after reopening.
- Exercised unsaved Save/Discard/Cancel. A discovered native Command-Q bypass
  was fixed with a custom guarded Quit item; Cancel retained the dirty game
  and Discard exited.
- Selected the already installed Stockfish 17 executable, analysed synthetic
  and real decoded PGN positions, switched positions rapidly, checked a black
  root with White-perspective scoring and applied MultiPV=2.
- Closed the engine and closed the native window during analysis; process
  checks found no remaining Stockfish child.

Cloud-backed original files sometimes blocked OS reads during hydration while
the UI remained responsive. A pending database open was superseded by a PGN
open; a regression test now verifies it cannot retain the editing lock.
Real-engine integration used the previously decoded real PGN during that cold
open. It does not claim that cold CBH loading finished in that engine run.

## Final bounded corpus comparison

The release-built Rust inspector was compared against cbh2pgn revision
`42b3592738062db1f768239e85df1b98cb1cead9` and python-chess on 2026-10-11.
The reference `game.py` SHA-256 was
`1d270a82e13dff63af0b1ef2a3efdd42d8196944f275d3aa9b39ee245cb2d7e6`.

Selection combined 39 retained overlap entries (36 games, three text entries)
with 200 deterministic spread ordinals: 238 unique entries. All 235 supported
games matched ordered UCI trees, initial FEN, date and result. Three text
entries were explicitly unsupported; there were zero comparison failures.
This is sample evidence, not exhaustive validation of millions of games or
original annotations.

Final run: maximum measured page time 5.62 ms, maximum game load 349.00 ms,
119,946 native bytes read across the comparison, peak inspector-child RSS
7.06 MiB. An earlier warm run measured 4.3/4.2 ms page/load maxima. Cloud reads
and cache state affect elapsed time. The reader uses bounded per-game reads and
an 8 MiB entity cache; no whole multi-gigabyte database is loaded into memory.
Private reports and game exports remain outside the source repository.

## Newer-format cross-check (2026-10-13)

The private Mega Database 2025 copy contains a classic family (11,151,119
entries) and a 2CBH family (24,783 entries) under the same name. Ordinal *n* in
the 2CBH family corresponds to classic ordinal *n*, with reassigned name ids.

The release-built `compare_formats` example (`crates/cbh-reader/examples`)
opened both families through the same `Database` API. It compared all 24,783
2CBH entries with the classic entries at the same ordinals:

* All 24 text entries agreed on kind, with no games decoded for them.
* All 24,759 games matched on these fields:
  * kind, date, result, round and ratings;
  * white, black and site names (2LID against CBP/CBT);
  * packed ECO;
  * the complete ordered move tree, including all 597 set-up games and
    1,807,103 move nodes.
* Events matched in 24,758 games. The one difference is in the data itself:
  "Donhead Consultation" in the classic file against "Donhead consultation" in
  2LID.
* No 2CBG block failed its marker, trailer or checksum. There were no
  unsupported tokens and no unexplained diagnostics. 63 games carry result
  codes 3–7, which both families report as uninterpreted.

Two classic-decoder corrections came out of this comparison: setup byte 0 = 1,
and an impossible en-passant file in entry 13409. Both are described in
[compatibility](compatibility.md). The whole 2CBH comparison took under 10 s
with a 1.4 ms maximum game load and about 10 MB read from the newer family.

This establishes agreement between two ChessBase encodings of the same games.
It does not cover 2CBH files written by other ChessBase versions, extended
move tokens or 2CBA annotations.

## Distribution and remaining acceptance

Git-tracked source paths are checked against an explicit allowlist. Private
home paths, database families, extracted binaries, archives and unapproved
PGN fixture names are rejected. The full locked external Rust dependency graph
and runtime npm graph have recorded licenses and checksummed upstream notices;
build outputs are excluded from source. No original ChessBase resources or
commercial game corpus is included. No engine is bundled.

The committed CI matrix configures `windows-latest`, `macos-latest` and
`ubuntu-24.04` native tests/static checks/builds and retains host metadata,
logs and executables. Official Linux GTK/WebKit prerequisites are installed.
The action revisions are pinned. The workflow has not been run here on hosted
Windows/Linux runners. Local Linux/Windows execution, signing/notarization,
packaged installers and native workflows on those OSs remain outstanding.

See [compatibility](compatibility.md) for unsupported CBA annotations, packed
ECO, newer-format limits, special encodings and standard-chess-only restrictions.
