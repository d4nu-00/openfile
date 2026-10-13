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
  codes 3–7; both families show the same strings for them (see below).

Two classic-decoder corrections came out of this comparison: setup byte 0 = 1,
and an impossible en-passant file in entry 13409. Both are described in
[compatibility](compatibility.md). The whole 2CBH comparison took under 10 s
with a 1.4 ms maximum game load and about 10 MB read from the newer family.

This establishes agreement between two ChessBase encodings of the same games.
It does not cover 2CBH files written by other ChessBase versions, extended
move tokens or 2CBA annotations.

## Annotation cross-check (2026-10-13)

The same `compare_formats` run decodes classic CBA and 2CBA annotations for
every shared game and compares the results node by node: comments, text
before moves, NAGs, squares and arrows.

* 24,723 of 24,759 games agree exactly. Together they hold 21,529 comments,
  4,705 texts before moves, 15,769 symbols, 238 coloured squares and 243
  arrows.
* The other 36 games contain 2CBA game-quotation or training entries, whose
  nested size is not recovered. The 2CBA reader stops there and reports it;
  the classic CBA path decodes those games fully.
* Classic entry 7289 has a text record that runs into the next record after a
  NUL byte. ChessBase's own 2CBA copy ends the text at that NUL, and so does
  the reader.

Status counts: 8,017 games are fully represented. 564 are `partial`, because
they hold types that are not shown or translations in other languages
(diagnostics give the counts). The rest have no annotations.

With annotations included, a warm run reads about 13 MB from the newer
family; its maximum game load is 0.9 ms. A cold first run measured 3.1 s for
one load while the cloud-backed file hydrated.

The position-to-move mapping was chosen statistically (see
[formats](formats.md)) and then confirmed by the agreement between the two
encodings. It is not a native trace of the numbering code.

## Names, results and the real app (2026-10-13)

The same comparison now also resolves annotators, sources and teams:

* Sources (title, publisher and date) matched in all 24,759 games, and teams
  in all 162 games that have them (CBJ and CBE against 2LID).
* 2,374 games name an annotator. Annotators matched in 24,719 games, after
  writing classic "Last,First" as "Last, First". In the other 40 games the
  classic CBC entry is "Mackenzie,George Henry", while the 2CBH annotator id
  points at the player entry "MacKenzie, George Henry".
* Result strings, including codes 3–7, matched in every game.

The run took 14 s. Its output is kept outside the repository with the other
comparison runs.

The real macOS app was opened at a richly annotated Mega game (2CBH entry
3763) using the command-line options described in the README. A WebKit
snapshot taken in a debug build showed the decoded arrow and coloured square
on the board, plus text before moves, comments and NAGs in the notation.

## Database writing (2026-10-13)

Writing was checked against the Mega sample from two directions. Outputs stay
outside the repository.

* **Move encoding.** The `encode_check` example re-encodes every 2CBH game of
  the sample from its decoded tree. 24,754 of 24,759 payloads are
  byte-identical to ChessBase's own. This confirms the token table's
  captured-piece order and ChessBase's chained variation markers. The other
  five are set-ups that store move number 0; the reader shows that as 1, and
  the writer stores 1.
* **Write and read back.** The `write_sample` example copied games into new
  databases in a temporary directory, read them back with the ordinary reader
  and compared every node's moves, comments, text before moves, NAGs, squares
  and arrows. It also compared players, events, sites, dates, rounds, results,
  ratings, ECO, annotators and sources.
  * 3,000 games from the 2CBH family (every seventh entry) all match. 2,999
    move payloads are byte-identical to the source.
  * 5,000 games from the classic family, spread across all 11.15 million
    entries (every 2,203rd), all match, including classic set-ups and CBA
    annotations. One game holds two move symbols; the second goes to the free
    position slot, as ChessBase stores $8 in the sample.
  * Two entries in each copy were replaced and marked deleted, and one was
    undeleted. Each read back as expected without changing the entry count.

  Writing 3,000 games in batches of 500 took 8 s.

Synthetic Rust tests cover:
* creating a database, saving, replacing and deleting;
* reuse of names;
* refusal of existing families and of classic paths;
* detection of files changed by another program;
* the registry's read-only and already-open guards.

Frontend tests cover New, Open for Editing, Save, Replace and Delete.

**Not yet verified:** opening these files in ChessBase itself. The files
ChessBase needs are described in [writing](writing.md), with a checklist for
that test.

## Game-data search (2026-10-13)

The `search` example ran whole-database searches on the Mega sample (release
build, warm cache):
* Kasparov against Karpov, colours ignored: 193 games from the classic
  family's 11,151,119 entries in 4.3 s.
* Carlsen wins since 2024 in B20–B99: 87 games in 3.7 s.
* Morphy wins in the 2CBH family: 320 games from 24,783 entries in 0.23 s.

Only the entry index and the name tables are read; no game is decoded.
Synthetic tests check each field, the colour and wins rules, deletion, the
hit limit and cancellation in both families.

Position search on the 2CBH family (24,783 entries, warm cache):
* Building the search booster took 4.5 s (3.2 MB), about 0.18 ms per game.
  The 11.15-million-entry classic family would take about 35 minutes.
* The Ruy Lopez after 3.Bb5 is reached in 3,700 games. With the booster,
  11,009 candidates were replayed in 1.15 s; replaying every game took
  2.5 s. Both runs found the same games at the same nodes.
* The final position of Morphy–Duke of Brunswick: 1 game, 65 candidates,
  0.01 s with the booster.

Synthetic tests compare booster and full-replay results, including
transpositions, positions only in variations and the side to move. They also
check that a stale booster is not used and that a cancelled build leaves no
file. In the real app, Find Position on the Opera Game after 4...Bxf3 listed
9 games, each opening at that move.

## Opening tree (2026-10-13)

The `tree` example built and queried trees on the Mega families (release
build, 8 decoding threads, development Mac with 10 cores and 16 GB):

| Family | Build | File | Stored positions | Peak memory |
|---|---|---|---|---|
| 2CBH, 24,783 entries | 3.3 s | 0.8 MB | 8,085 | — |
| Classic, 11,151,119 entries | 585 s (385 s decoding, 200 s sorting in 19 passes) | 686 MB | 3,576,062 | 0.8 GB |

The classic build's temporary file of packed main lines is deleted when the
build ends.

* **Answers.** A stored position is answered in under a millisecond. The start position and
  the Najdorf after 6.Nc3 a6 each listed their first 100,000 games in 11–25 ms.
  The Najdorf has 61,217 games for 6.Bg5. Positions from three late classic
  games at 16, 30 and 60 half-moves, mostly off the stored positions, were
  followed from the line in 0.3–1.6 ms.
* **Exactness.** `--verify 3000` compared 3,000 random lines cut from the 2CBH
  family's games with a scan of every main line. 2,965 were exact. The other
  35 missed games that transpose into a position reached by fewer than five
  games, or started from a set-up position. No answer listed a wrong game.
* **Synthetic tests** check counts, transpositions, variations (not counted),
  scores from the side to move, unknown results, ratings, years, following
  rare positions, stale trees, journalled saves and replacements, and that a
  cancelled build leaves no file. The app test also checks the reference
  search from the tree, its fallback to Find Position, and found nodes.

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

See [compatibility](compatibility.md) for annotation types not shown, packed
ECO, newer-format limits, special encodings and standard-chess-only restrictions.
