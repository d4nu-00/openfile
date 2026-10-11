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
ECO, newer formats, special encodings and standard-chess-only restrictions.
