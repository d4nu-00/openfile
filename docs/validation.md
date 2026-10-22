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

The private the test database copy contains a classic family (11,151,119
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

This establishes agreement between two encodings, by the reference app, of the same games.
It does not cover 2CBH files written by other versions of the reference app, extended
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
  NUL byte. The reference app's own 2CBA copy ends the text at that NUL, and so does
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

The real macOS app was opened at a richly annotated test database game (2CBH entry
3763) using the command-line options described in the README. A WebKit
snapshot taken in a debug build showed the decoded arrow and coloured square
on the board, plus text before moves, comments and NAGs in the notation.

## Database writing (2026-10-13)

Writing was checked against the test database sample from two directions. Outputs stay
outside the repository.

* **Move encoding.** The `encode_check` example re-encodes every 2CBH game of
  the sample from its decoded tree. 24,754 of 24,759 payloads are
  byte-identical to the reference app's own. This confirms the token table's
  captured-piece order and the reference app's chained variation markers. The other
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
    position slot, as the reference app stores $8 in the sample.
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

**Not yet verified:** opening these files in the reference app itself. The files
The reference app needs are described in [writing](writing.md), with a checklist for
that test.

## Game-data search (2026-10-13)

The `search` example ran whole-database searches on the test database sample (release
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

The `tree` example built and queried trees on the test database families (release
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

## Game editing (2026-10-13)

`chess-core` tests delete and promote nested variations and check that node
ids stay dense and consistent. They also cover deleting the remaining or
previous moves (the new start position's FEN, en passant square included),
text before moves, and game data checks: result, ratings, date form, tag
names and one-line values. The start position's SetUp and FEN tags cannot be
edited by hand. App tests cover undo and redo across structural edits (a new
edit clears redo), new games, set-up positions (an impossible one is refused),
pasted PGN (plain text is refused) and copying the game as PGN. Frontend tests
drive the shortcuts, the Game data and Position setup dialogs, the
Annotation Palette and the paste fallback dialog.

## Game list: scrolling and sorting (2026-10-13)

The `sort` example built sort orders on the test database sample (release build, warm
cache, 8 threads). The classic family's 11,151,119 entries sorted by White in
0.84 s, by Date in 0.33 s and by Tournament in 0.67 s. Each took 188–220 MB at
peak and wrote a 44.6 MB file. The 2CBH family's 24,783 entries took
0.06–0.26 s. The first and last rows were checked by eye: text entries came
first, then the earliest dates, Zukertort's and Zytogorski's games at the end
of the name orders, and a tournament's games in round order. Finding the row
of an entry in the middle of the classic order took 5–20 ms. See
[game list](game-list.md).

Reader tests build orders for every column of a synthetic 2CBH database. They
check:
* names with folded case and accents;
* that ties keep entry order;
* ascending and descending reads;
* finding an entry's place;
* arranging a search result;
* that the file is stale after a save or for another column;
* that a cancelled build leaves no file;
* the entry limit.

A synthetic classic family covers the record offsets and puts text entries
first. App tests read rows of the plain, searched and sorted list. They check
that a save drops the sort and that row bounds are enforced. Frontend tests
cover:
* wheel scrolling and block reads;
* Go to entry;
* keyboard selection;
* the header sort cycle and a cancelled sort;
* the quick filter;
* that Load Next Game follows the list;
* that the row cache ignores stale answers and keeps 60 blocks at most.

## Players, tournaments and crosstables (2026-10-13)

The `names` example on the classic test database (release, warm) listed 463,258
players in 0.58 s and 105,313 tournaments in 0.13 s. It found "Carlsen, M" in
0.5 ms; Carlsen, Magnus has 6,925 games from 1999 to 2024. It built the
crosstable of Tata Steel TOP 81st (Wijk aan Zee 2019, 10 players, 45 games,
9 rounds) in 3.5 s. The scores and Sonneborn-Berger values were checked by
hand: the winner's 7 points and SB 29.00 agree with the opponents' scores.

**Tests.**
* Reader tests check:
  * player and tournament lists, game counts and years;
  * the prefix search;
  * a cancelled build and staleness;
  * a round-robin table's points, ranks, cells, Sonneborn-Berger, Buchholz,
    performance, average Elo and category;
  * that an unscored game is excluded;
  * the forced Swiss layout's opponents and rounds.
* App tests check that lists must be prepared before reading, the bounds,
  searches by player id, the crosstable through the registry, and that
  saving drops the lists.
* Frontend tests drive the Players tab: find, choose, and open the games.
  They also drive the Tournaments tab, the Cross Table dialog, its layout
  switch and opening a game from a result.

## Several databases, copy and paste (2026-10-13)

`cbh-reader/examples/copy_games.rs` copies games the way Paste does, then
reads every copy back and compares it. Into a new 2CBH file in the scratch
folder, it copied:
* 2,000 games of the classic test database in 1.0 s;
* 20,000 games in 9.9 s, about 0.5 ms per game, warm.

All copies matched their source in names, event, result and number of
move-tree nodes. The first run found a "0-0" game stored back as a line. The
`CBResult` tag fixes that. The file was deleted afterwards.

**Tests.**
* A write test covers the kept forfeit and 0-0 codes, and that an edited
  Result wins.
* App tests cover:
  * copying rows of a sorted list;
  * pasting with progress;
  * moves and comments arriving in the copy;
  * refusals for an empty clip and for the source database;
  * batch delete and undelete.
* Frontend tests cover:
  * the selection ranges;
  * a Shift-click selection copied and pasted into a database opened for
    editing in a second tab;
  * Select All with Delete;
  * keeping the game and the other database when another opens;
  * switching between and closing database tabs.

The paste was not driven in the real app: the app's debug startup options
open only one database.

## Opening games and drawing (2026-10-13)

`src-tauri/examples/open_timing.rs` timed opening games of the classic test database
in a release build: `load_entry` takes 0.3–0.8 ms on average, the book at
most 4 ms and a tree-answered reference search at most 22 ms. A Find
Position without a search booster, the reference pane's old fallback, took
1,357 s. The details and the fixes are in [game window](game-window.md).

**Tests.**
* App tests cover:
  * a reference search on a "large" database (the limit lowered in the test)
    that reports `notSearched` and searches when asked, while the tree
    still answers;
  * a game loaded, replaced and loaded again through the game handle;
  * drawn marks: edit, undo, redo, PGN `[%csl]`/`[%cal]`, saving into a 2CBH
    database and loading back, and Delete Colour Commentary.
* Core tests cover mark validation, deduplication and marks on the start
  position.
* Frontend tests cover:
  * right-click squares with Shift, an Alt+Ctrl drag arrow, and the swallowed
    click;
  * adding, recolouring and removing marks;
  * a double-click that loads its game once;
  * the reference pane's Search all games.

Drawing was not driven in the real app: its debug snapshots cannot press
mouse buttons.

## Game tabs (2026-10-13)

**Tests.**
* App tests cover the settings file: the reference app's default of 8 tabs, checks
  of the values, a round trip, and a damaged file read as the defaults
  without being overwritten. The document budget test now allows 16.
* Frontend tests cover:
  * two games opened from the list in two tabs, each keeping its game and
    unsaved state;
  * closing a tab with unsaved changes (Cancel, then Discard), and showing
    the last used tab after it;
  * at a maximum of 2 tabs set in Options, the least recently used clean
    tab reused, and a game with a tab shown in it;
  * quitting with two edited tabs: Cancel on the second keeps the app open,
    and discarding both quits;
  * New Game opening a second tab;
  * the tab state functions.

A debug build on the classic test database opened entry 27878 at start, then ran
Board Window: the snapshot shows both game tabs beside the database tab.

## Engines (2026-10-13)

**Tests.**
* App tests start 4 kibitzers of the engine crate's mock UCI engine at once,
  refuse a fifth, close one and then all, and probe the engine's name. They
  paste an engine line as one edit (a variation beside the game's move, the
  comment on its last move), undo it, and refuse an illegal line without a
  change. Core tests cover following existing moves and the unchanged game
  after an illegal move.
* Frontend tests cover:
  * + and − setting MultiPV, stopping and restarting the analysis;
  * Ctrl+Space pasting the best line with `+0.35/22`, and Shift+Space its
    first move;
  * two kibitzers, Remove Kibitzer taking the last one used, and Remove All;
  * installing an engine (named from its handshake, made the default) and
    Alt+F2 starting and removing it;
  * the earlier engine tests, now with kibitzers that start at once.

A debug build on the classic test database ran Default Kibitzer with Stockfish 17
installed in a temporary settings file with `MultiPV` 3: the snapshot shows
the pane analysing node 30 with its lines. The settings file was deleted
afterwards.

## Tablebases (2026-10-13)

`src-tauri/tests/tablebases.rs` probes real Syzygy files when `SYZYGY_DIR`
names a folder of them; none are in the repository. Run against a local
folder of 3- to 5-piece tables (96 files) outside the repository:
* K+Q v K with White to move is a win with a positive DTZ; winning moves
  come first, ordered by DTZ, and Qe4+, which loses the queen, is a draw
  and comes last;
* K+P v K with the black king in front of the pawn is a draw;
* the start position has too many pieces.

Without `SYZYGY_DIR` the test of real tables reports that it was skipped;
the no-tables, missing-folder and invalid-position cases always run.
Frontend tests cover the Tablebase tab's result and moves, playing a move
from it, and the folder chosen in Options. The real app was not shown a
tablebase position: its debug startup cannot set up a position.

## Tactical Analysis (2026-10-13)

`src-tauri/tests/blunder_check.rs` runs Blunder Check with the engine
crate's mock engine, which gives every position +0.17 and the line e7e5: on
1.e4 e5 it finds 1.e4 (a loss of 34) at a threshold of 30, and nothing at 35
or for Black only; a cancelled check reports nothing; bad parameters are
refused. The findings applied as one edit give
`1. e4 $2 {+0.17} (1. d4 d5 {+0.17/12}) 1... e5`, and an item that does not
apply changes nothing.

With a real engine (`UCI_ENGINE`; run with Stockfish 17 at 300 ms per
position, threshold 1.00, Black's moves), the Scholar's mate game
`1. e4 e5 2. Qh5 Nc6 3. Bc4 Nf6 4. Qxf7#` gives exactly one finding:
3...Nf6, after which White mates in one (`+M1`), with another move in the
engine's line. Frontend tests cover the dialog's parameters (Time × 500 ms,
Fine = 35), the "??" rule, the one edit, and a stopped check adding nothing.

## Names with figurines (2026-10-13)

Of the classic test database's 463,262 players, 362 name fields are not ASCII and
some were not valid UTF-8; their bytes were 0xA4 (323), 0xA5 (289), 0xA3
(111), 0xA6 (43), 0xA2 (10), 0xA0 (3) and 0xAA (1). The 0xA2–0xA6 names are
opening lines ("1.b3 e5 2.?b2 ?c6 3.e3", "10...?b8"), where the bytes are
the pieces K, Q, N, B, R of the reference app's figurine fonts: "?b2" after b3 e5 is
Bb2, "?c6" Nc6, "ed5 ?d5" Qd5, "?fd8" Rfd8, "?b1" Kb1. 0xA0 occurs in real
names ("Rodrigo Pereira") as Windows-1252's no-break space. Classic names are
now read as UTF-8 when valid, otherwise as Windows-1252 with 0xA2–0xA6 shown
as ♔ ♕ ♘ ♗ ♖; they no longer show "�". The meaning of 0xAA (once, in an
opening line) is not known; it shows as "ª".

## Crosstables through a sort order (2026-10-14)

`cbh-reader/examples/crosstable.rs` timed crosstables of the classic test database
both ways (release, warm): the tournament of entry 27878 (18 players) took
3.45–3.55 s by scanning the entry index and 4–9 ms through a sort order by
Tournament; that of entry 5,000,000 (174 players) 3.46 s and 46 ms. The
tables were equal. Building the order took 0.69 s; it was written to a
scratch folder and deleted. A reader test compares both ways for every
tournament of its database.

## Opening tree after a paste (2026-10-14)

An app test builds a tree for six games of 1.d4 d5, pastes four 1.e4 games
from another database and asks the book for the start position: 10 games
from the tree, 4 of them with e4, and the paste does not report the tree as
stale.

## Player statistics (2026-10-14)

`cbh-reader/examples/player_stats.rs` scored three players of the classic
test database (release, warm). Carlsen: 6,925 games found in 0.056–0.066 s by 8
threads and scored in 0.04–0.06 s. As White he has +2045 =1006 −464 (72 %,
performance 2788); as Black +1551 =1222 −637 (63 %, 2724). Kasparov: 2,735
games in 0.063 s; Nakamura: 9,288 in 0.063 s.

A first version also found the games through sort orders by White and by
Black. On Carlsen, both ways returned the same games, but the sort orders
took 0.14 s, so they were dropped. The orders were built in a scratch
folder and deleted.

Reader tests check the scoring (colours, forfeits, unscored lines,
performance, ECO rows, opponent ratings, years) and that 1, 2, 5 and 16
workers find the same games in entry order. An app test scores a saved
database and lists a row's games. A debug snapshot of the real app showed
the dialog for Carlsen. It also exposed duplicate "no ECO" rows from codes
above E99, which now count as none.

## Search mask and position search (2026-10-14)

The measurements are in [search](search.md#measurements-classic-mega-2026-10-14).

* **Parallel scans.** On the classic test database the parallel game-data search, Find
  Position and booster build returned the same results as before. The
  Ruy Lopez search found the same first hits (entries 231, 410, 412, 476,
  730, each at node 5). The Opera Game position gave the same 1,035 hits
  with and without the booster.
* **Source reads.** The first parallel Find Position took 651 s for the
  Ruy Lopez. `time` showed more system time than user time, and on the 2CBH
  test database one worker spent 2.0 s in the kernel against 0.7 s of chess work.
  Positional reads and a path check at most every 200 ms brought it to
  108 s. A per-read `fstat` was also tried: it cost 1.7× on eight workers
  and was dropped.
* **Booster.** It was built into a scratch folder in 128 s (1.43 GB),
  measured, and deleted.
* **Reader tests.**
  * final positions (mate, stalemate, check that is not mate), full text in
    comments and variations, NOT, and hit limits keeping the first hits in
    entry order;
  * patterns: 'Look for', 'Or' and 'Exclude' boards, any piece, several
    pieces on a square, both mirrors, First/Last/Length, side to move,
    variations;
  * exact positions with mirrors, with and without the booster;
  * refused queries.
* **App test.** A pattern from JSON, its progress and found nodes, and an
  empty query refused.
* **Frontend tests.** The Position tab (exact from the board, then a
  pattern with the Exclude board and mirroring, reopened with the same
  boards), and progress with Cancel in the status bar.
* **Demo app.** The tab was driven in the browser pane on demo data.

## Chess960 and special CBG records (2026-10-14)

A census of the classic test database's CBG headers counted:
* 892 Chess960 records with a set-up (`0x4a`) and one without (`0x0a`);
* 5 records with flag `0x04`, 4 with `0x05` and 2 with `0x01`.

`cbh-reader/examples/entries.rs` loaded each listed entry:

* **Chess960.** All 892 games load (77,149 nodes), and every move is legal.
  The two games checked by hand match their own comments. Bytes 38–39 read
  748 and 811, as do their "Chess 960-Position" comments. Both play the
  castles the scores show (Leko–Adams, 11...O-O-O and 12.O-O-O). The record
  without a set-up is refused.
* **Special records.** All 11 load (800 nodes): the five kind-4 games, the
  four kind-5 games and the kind-1 game each decode every byte as legal
  moves, and the empty kind-1 record gives a game without moves.
* **How the encodings were found.**
  * Byte statistics showed the move stream of Chess960 records starts at
    byte 40: byte 39 takes 149 values, byte 40 only 18.
  * Trying each 256-byte table of `0x1401fb8f0` gave legal openings only
    for `0x141975610` (Chess960) and `0x141974a10` (kind 4).
  * The two-byte-word layout was read from `0x1401fc8b0`.
* **Tests.**
  * Reader tests encode Chess960, kind-4, kind-5 and kind-1 records with the
    tables, and check that wrong castling squares and words with variations
    are refused.
  * chess-core tests: Chess960 castling, SAN, `Variant` and a PGN round
    trip.
  * An engine test checks that `UCI_Chess960` is switched on before a
    Chess960 position and off after it, and that an engine without the
    option is refused.
* **Real app.** A debug snapshot showed Leko–Adams replayed from the
  classic test database.

## Tablebases in the notation (2026-10-14)

* **Tests with real tables** (`SYZYGY_DIR` set to a local folder of 96
  files, 3–6 pieces): from K+Q v K, 1.Qe4+?? Kxe4 gets "?", "Tablebase:
  draw" and a winning variation, while 1.Qh5+ Kd4 gets nothing. Without
  tables nothing is added.
* **App test.** It checks that a mock engine offering `SyzygyPath`
  receives the chosen folder, and that one without the option does not.
* **Frontend test.** The command applies the items as one edit.
* **Real app.** A debug snapshot used a scratch 2CBH game (1.Qh5+ Kd4
  2.Qd5+ Kxd5) and a temporary settings file naming the folder; both were
  removed afterwards. It showed 2.Qd5+? with "Tablebase: draw" and the
  variation 2.Qb5 "Tablebase: White wins".

## Database start screen (2026-10-14)

* **App tests.**
  * Opening records Recent (latest first, at most 16) and My Databases (once).
  * Remove Database Symbol keeps the files; pins and types work, and an
    unknown type is refused.
  * Folder shortcuts list only `.cbh`/`.2cbh` files, by name; missing files
    show as missing.
  * The lists are only in the settings file.
  * Relative paths, duplicates and over-long lists are refused.
* **Frontend tests.**
  * The screen shows with no database open, opens a symbol on double-click
    and is brought back by My Databases.
  * Missing databases do not open.
  * Type, Remove, Pin, folder listing, Add/Remove Folder Shortcut, Details
    and Folders work.
* **Real app.** A debug snapshot used a temporary settings file holding both
  test database families, a missing database and the test database's folder, and the file was
  deleted afterwards. The screen showed 11,151,119 entries (CBH) and 24,783
  (2CBH), read from the header records alone. The first attempt was taken
  before the cold first header reads had finished, so the screen now says
  "Reading the lists…" until they arrive.

## Keyboard symbols and Tactical Analysis symbols (2026-10-14)

* **Evidence.** The symbol popups, their order and the score-to-symbol
  table were read from the reference binary statically. They are listed in
  [game window](game-window.md#annotation-symbols-from-the-keyboard) and
  [engines](engines.md#tactical-analysis-blunder-check).
* **test database check.** All 24,759 annotation blocks of the test database's 2CBA file were
  read once, with a scratch example that was not kept, to count symbols per
  slot. "Only move" is NAG 8 (34 games) and "=" mostly 11 (904 games,
  against 137 for 10). No game has Zugzwang.
* **Frontend tests.** Typing "?" opens the move symbols on the move's
  current symbol; Down and Enter replace "!" with "?". "=" adds an
  evaluation beside the move symbol, Escape closes, and "(none)" clears
  the group. The palette replaces a symbol within its group. Tactical
  Analysis adds evaluation symbols in the same edit as its findings.
* **Rust tests.** The score-to-symbol table, the change and repeat rules,
  and an evaluation replacing "=" but not "with attack". Move, position
  and prefix symbols (8, 13, 142, and 22 beside them) are storable in
  2CBH.

## Convert to 2CBH (2026-10-14)

* **Measurements.** `cbh-reader/examples/convert.rs` converted three
  50,000-entry slices of the classic test database and the whole 2CBH test database into the
  scratch folder. The outputs were deleted afterwards. Times, sizes and the
  estimate against the size written (1.11–1.57×) are in
  [conversion](conversion.md#measurements). Chess960 games (42 in the last
  slice) and text entries were left out and counted.
* **Reader tests.** Deleted marks survive a conversion, games keep their
  moves and annotations in the order asked for, and a cancelled conversion
  writes nothing.
* **App tests.** A whole database and a selection convert into new
  databases. The progress runs from 0 to all, and no temporary files
  remain. The source's files keep their sizes and times. An existing
  family file (including `.2lcd`), or the source itself as target, is
  refused.
* **Frontend tests.** The dialog estimates the selection first and again
  after switching to the whole database. Convert opens the new database for
  editing and reports the counts. An estimate that does not fit disables
  Convert and says why.

## Database maintenance (2026-10-14)

* **Measurements** (`cbh-reader/examples/maintain.rs`, release, warm):
  * Check Integrity on the 2CBH test database found no problems (0.18 s).
  * On the classic test database it took 133.8 s and found 16 problems in
    11,151,119 entries. 12 are set-ups whose castling rights the board
    does not allow, which the reader refused and earlier spot checks had
    not met. Two are damaged move records. One is the known Chess960
    record without a start position, and one is an annotation record that
    does not parse. (This first listed 14 castling set-ups: the example
    prints only the first five problems, and the two damaged move records
    were counted with them. The 2026-10-15 re-run below lists them all.)
  * Find Double Games marked 4 games of the 2CBH test database with the defaults.
  * Remove Deleted Games on a scratch copy with every tenth game marked
    removed 2,476 entries in 0.41 s, and Check Integrity afterwards was
    clean. The copy was deleted. See [maintenance](maintenance.md#measurements).
* **Reader tests.**
  * Compaction:
    * keeps the other entries' moves, comments and variations, drops
      deleted entries and replaced games' old blocks, and renumbers;
    * removes `.2lcd`;
    * does nothing when cancelled, and refuses to overwrite a leftover
      backup.
  * A damaged 2CBG block is reported for its entry.
  * Doubles:
    * Similar names pair "Kasparov, G" with "Kasparov, Garry";
    * the annotated game is kept, then the rated one;
    * Always second and the game length work.
* **App tests.**
  * Integrity, doubles and compaction work through the registry.
  * A game open in a tab follows its new entry number, and Replace writes
    the right entry afterwards.
  * The database can be written after the swap, and no temporary or
    backup files remain.
  * Read-only databases can be checked but not marked or compacted.
* **Frontend tests.** The three dialogs and their results; doubles pass
  the chosen options.

## Mark Games and selection edits (2026-10-15)

* **test database check.** The medal word at 2CBH bytes 130–131 equals classic
  bytes 37–38 read big-endian in all 8,790 records of the first 24,783
  entries that have medals. The user medal (bit 15) is set in 11 games.
* **App tests.**
  * Mark Games toggles each selected game on its own, and Replace keeps the
    medal.
  * Edit Game Data sets the tournament (event and site) and the source, and
    clears the annotator of the selection only.
  * Moves are unchanged.
  * An empty request or a name with a line break is refused.
* **Reader tests.** A conversion keeps each entry's medal word.
* **Frontend tests.** "+" marks the Shift-selection and reports the reference app's
  message. F2 opens the dialog, which sets only the ticked field for the
  same selection.

## Notation editing batch (2026-10-15)

* **Core tests.**
  * Insert Into Game puts 2. h3 h6 before 2. Nf3 Nc6, keeping the
    variation 2... d6. Ids stay in pre-order.
  * An odd number of moves, or later moves that no longer fit (2. Qh5 Nc6
    then Nc6 again), changes nothing.
  * Delete Commentary by kind: text, evaluation and time (scores and
    `[%clk]`/`[%eval]`/`[%emt]`), and all, which also removes variations
    and selects the nearest remaining ancestor.
  * Null moves are refused in check.
* **Frontend tests.** Fold All, [+1] and Ctrl+-/Ctrl++ hide and show the
  variation. Ctrl+Alt+0, Insert Into Game and Ctrl+Shift+Y send their
  edits.

## Deep Analysis and analysis jobs (2026-10-15)

* **Unit tests** (`deep_analysis.rs`):
  * The reference app's schedule: 4 s, depth 2 and widths 3/1 at iteration 0; 7 s,
    depth 3, 4/2 at iteration 1; 6,735 s, depth 5, 5/3 at iteration 14.
  * Time per position: half as much again at the root, 0.7 at the deepest
    ply, at least 3 s.
  * The sibling rule, the "one more than ? and ??" rule, and the 70/70 cut.
  * A scripted analyst builds iteration 0's tree: 3 lines of 3 moves from 7
    positions. Stopping keeps a bounded tree with a best line.
* **App tests** with `mock-uci legal`, a mock mode that scores the legal
  moves of any position:
  * Deep Analysis builds a tree that goes in as one edit and comes out
    with one Undo.
  * Cancel returns nothing.
  * A time limit ends it; a missing engine is an error.
  * The jobs queue replaces two games of a new 2CBH database with two
    engine lines each.
  * A game open in a tab and a game changed since its job was added are
    skipped with their reasons. A Deep Analysis job goes in, and a
    restarted job runs once the tab is closed.
  * The reference app's minimum times (15 s, 30 s), the 1,000-job bound, Skip,
    Remove, New analysis jobs, and refusal on a read-only database.
* **Real engine** (`UCI_ENGINE=/opt/homebrew/bin/stockfish`, start
  position, 100 ms base and a 3 s limit): 2 iterations, 24 positions,
  6 lines, best line 1.Nf3 d5 2.d4 Bf5 (+0.31). The lines go in as one edit.
* **Frontend tests.**
  * Deep Analysis sends the node and time limit, adds the lines as one
    `addLines` edit, and reports the iterations.
  * Progress shows the iteration, positions, moves, time and best line;
    Cancel adds nothing.
  * Create analysis jobs sends the selection and the settings, then Start
    runs the queue.
  * The time warning holds the reference app's minimum.
  * Add Analysis Job sends the board position; Restart sends its action.

## Opening classification (2026-10-15)

* **Core tests** (`chess_core::eco`):
  * every line of the pinned lichess table replays and is indexed;
  * the Najdorf is B90 after 5...a6 (ply 10), and 6.Be3 is a deeper named
    line;
  * moves past the table keep the last match;
  * the Nimzo-Indian is E20 by either move order;
  * a game is classified by its main line, not its variations;
  * set-ups are not classified.
* **Reader tests** (`tests/eco.rs`):
  * Set ECO Codes classifies five games (Petrov, Najdorf, a mis-coded
    Queen's Gambit, a set-up, a Petrov after a variation) into B90, D35 and
    C42 changes, one unchanged and one unclassified;
  * "Only games without a code" keeps the wrong code;
  * the written codes read back, and a reference app sub-classification on an
    unchanged code survives;
  * a word beyond E99 is refused before anything is written;
  * a cancelled run returns nothing.
* **App tests:**
  * Classify Opening names the shown game (C42, Petrov's Defense, ply 6)
    beside its own ECO tag;
  * Set ECO Codes writes the codes of a new 2CBH database (only missing,
    then all) and refuses a read-only database.
* **Frontend tests.**
  * Ctrl+Alt+C shows the code, name and move, and Set ECO sends one
    `setTags` edit with the code.
  * Set ECO Codes starts with "Only games without an ECO code" ticked and
    reports the counts.
* **Measurements** on the test database, read-only: see [openings](openings.md).
  * 2CBH family: 24,759 games in 0.26 s, the same main code as the reference app
    for 76.4 % of coded games.
  * Classic: 11.15 M games in 188.5 s with 8 workers, 68.6 %.

## Opening report (2026-10-15)

* **App test** on a new 2CBH database of 62 games:
  * 60 reach 1.e4, with results 10/20/30;
  * history: the earliest (1990) and latest (2019) games, years, and
    5-year fashion buckets against the database's years;
  * players: the 2700 player as Black at 75 %, and the reference app's notable
    rule picking the two regular players over twenty one-game club players;
  * move rows from the games read (c5 40, e6 20, with "played by" and
    scores), then from the opening tree with performance once one is built;
  * the main line 1...c5 2.Nf3 d6 3.d4;
  * the two 1.d4 games listed individually.
* **Reader test:** years from the reader's dates.
* **Frontend test:** the report shows its title, line, results, history,
  year chart, players, moves with "played by", and the main line numbered
  after the position.
* **Measurements:** see [openings](openings.md#measurements-1). The 2CBH test database
  report of 1.e4 c5 takes 4.5 s; the classic test database's Najdorf takes 23.8 s
  warm.

## Variation tools and medal names (2026-10-15)

* **Core tests** (`chess_core::variations`):
  * Exchange Moves swaps 1...c5 with 1...e5, keeping 1...e6, and refuses
    a main-line move or a variation's later move.
  * Reorder Variations puts the 6-move Sicilian before the 2-move line
    when the main line is included, and only reorders inside variations
    otherwise.
  * Clean up variations merges 1.c4 e6 2.d4 Nf6 3.Nc3 Bb4 4.Qc2 into the
    main line's Nimzo-Indian position: 6 moves dropped, 4.Qc2 and its
    comment kept. A variation that never transposes stays as it is.
* **App test:** the three edits go through the document service as JSON.
  This caught `mainLine` being ignored, since the `Edit` enum renames
  variants but not fields.
* **Reader test:** medal names by bit.
* **Frontend tests:** the ribbon commands and the "Including the mainline?"
  question send their edits; a game's medals are named in the list.
* **test database check** (read-only): 62 medalled 2CBH records, mostly Endgame
  (22) and Best game (13).

## Distribution and remaining acceptance

Git-tracked source paths are checked against an explicit allowlist. Private
home paths, database families, extracted binaries, archives and unapproved
PGN fixture names are rejected. The full locked external Rust dependency graph
and runtime npm graph have recorded licenses and checksummed upstream notices;
build outputs are excluded from source. No original resources of the reference app or
commercial game corpus is included. No engine is bundled.

The committed CI matrix configures `windows-latest`, `macos-latest` and
`ubuntu-24.04` native tests/static checks/builds and retains host metadata,
logs and executables. Official Linux GTK/WebKit prerequisites are installed.
The action revisions are pinned. The workflow has not been run here on hosted
Windows/Linux runners. Local Linux/Windows execution, signing/notarization,
packaged installers and native workflows on those OSs remain outstanding.

See [compatibility](compatibility.md) for annotation types not shown, packed
ECO, newer-format limits, special encodings and standard-chess-only restrictions.

## Set-ups with impossible castling rights (2026-10-15)

* **Finding.** Twelve classic test database set-ups store castling rights that the
  board does not allow (entries 64687, 64740, 68207, 96531, 96535 and seven
  others): games taken up in the middle, mostly from 1934–1946, with K and k
  (bits 2 and 8, or 2 and 4) set while the kings stand on g1/g8 or elsewhere.
  The reader refused them as "invalid castling rights".
* **Change.** A right whose king or rook is off its standard square is
  dropped, one right at a time, after the set-up as stored has been tried
  (`setup_game` in `cbh-reader/src/moves/setup.rs`, shared by classic and
  2CBG set-ups). Standard records are read as standard chess before
  Chess960, so a stored right is never taken for a Chess960 one. A
  temporary counter over the whole classic test database found no standard record
  that only the Chess960 reading accepted, so no other game changes.
* **Result.** All twelve now decode to their last move with plausible
  play. Check Integrity on the
  classic test database, release build, 8 workers, warm: 157.9 s with a Ghidra
  analysis running beside it, 4 problems instead of 16, namely two damaged
  move records (3079389, 7168922), the Chess960 record without a start
  position (489834) and the annotation record (2455498).
* **Not traced.** How the reference app itself treats these rights: its 2CBH copies
  of these games are outside the 24,783-entry 2CBH sample, and its set-up
  validation was not followed.
* **Tests.** `setups.rs` builds set-ups with kings on g1/g8 and K, k set
  (both dropped), and with one possible and one impossible right (the
  possible one kept, and castling with it decodes).

## Prepare against White or Black (2026-10-15)

* **Measurements** on the classic test database, read-only: see
  [preparation](preparation.md#measurements). Player A with White: 3,515
  games, all readable; 14 repertoire lines.
* **Tests.** Reader tests answer positions from kept main lines, follow the
  repertoire threshold and average the recent rating. An app test prepares
  against a player of a scratch 2CBH with each colour, answers the Book,
  lists all games and those of a line, refuses another colour, and drops the
  preparation when it is closed or the database changes. Frontend tests run
  Ctrl+W and Ctrl+B from the Players list, switch colour, list a line's
  games, show the preparation in the Book and return to the database tree.

## Player dossier (2026-10-15)

* **Measurements** on both Megas, read-only: see
  [player dossier](player-dossier.md#measurements).
* **Tests.** Reader tests check the reference app's band tables, the per-colour
  tally (score rounding, performance, game lengths, draws and quick draws,
  forfeits left out) and the score by rating band. An app test builds the
  dossier of a scratch 2CBH player: score bands by colour, quick draws,
  rating bands, a mate given and one received, a medal set in the record,
  a self-annotated game, the White repertoire, and listing the kept games.
  Frontend tests open it with Ctrl+Alt+D, check the workbench's wording
  and that missing values show as nothing, list an opponent's games and
  load a mate.
* **Real app.** A debug snapshot of a 446-game player of the 2CBH test database
  showed "null%" for missing values; the dialogs (and Player statistic's
  score) now treat null as missing.

## Tournament report (2026-10-16)

* **Measurements** on the classic test database, read-only: see
  [tournament report](tournament-report.md#measurements).
* **Tests.** An app test reports a scratch 2CBH tournament of three players:
  rounds in order with the unnumbered game last, the earliest date, the
  pairings, the standings after each round (ties in crosstable order), the
  result statistics, and listing a round's games with `show_entries`, which
  refuses entries out of range. Frontend tests open the report from the
  Tournaments list and from the Cross table, apply the round range, trim
  unknown date parts, list a round's games and load a game.
* **Real app.** A debug snapshot of a 17-round match of the 2CBH test database showed
  dates as "1905.??.??"; unknown parts are now left out.

## Error Report (2026-10-16)

* **Measurements** with Stockfish 17 on the classic test database, read-only: see
  [error report](error-report.md#measurements). The first version's
  estimates were 400–700 points low; tracing showed the reference app divides the
  plain losses by weights of at least 2 for ordinary moves, and the
  corrected version came within 82–292 points on three strong players'
  latest games.
* **Tests.** Unit tests check the reference app's Blunder Elo curve, the error
  value, the bands, severity, distribution, transitions, phase and opponent
  strength. An app test runs `mock-uci legal` over a scratch 2CBH: one
  player's moves or both sides', a game without the player skipped, losses,
  the error value, the rating, too few moves for an estimate, Stop, and an
  empty selection refused. Frontend tests run the dialog with a player and
  with both sides, and load a worst move.

## Annotation and medal search (2026-10-16)

* **Measurements** on the classic test database, read-only: see
  [search](search.md#annotations-and-medals-tabs-2026-10-16). Each search
  took about 1 s, reading only the annotation records of annotated games.
* **Tests.** A reader test writes a scratch 2CBH with texts, symbols,
  coloured squares and arrows, a variation and a user medal, and checks
  each criterion (two texts, whole words, symbols, any text or symbol,
  colours, variations, medals). Another checks the critical-position kinds,
  training and multimedia on parsed entries. A frontend test opens Search
  text on the Annotations tab and searches with symbols, colours, a
  critical position and two medals.

## Databases in iCloud Drive (2026-10-18)

* **The report.** Opening the classic test database "took forever". The backend's
  open (`src-tauri/examples/open_timing.rs`, release build) had not returned
  after ten minutes. A stack sample showed it blocked in `pread` with no CPU
  time, reading a name table's header.
* **The cause.** The database lies in `~/Documents`, which iCloud Drive
  syncs, and "Optimize Mac Storage" was on. After the disk filled up on
  2026-10-16, macOS had removed the contents of most of its files
  (`ls -lO` shows `dataless`): 5 of the 9 files the classic reader opens,
  1.49 GB including the 1.25 GB move file, and all 4 files of the 2CBH
  family. Reading such a file waits until macOS has downloaded all of it.
  `brctl status` showed the download failing ("Network Failure … The
  request timed out", three attempts); a 32 KB file requested with
  `brctl download` had not arrived after a minute either.
* **The change.** `cbh_reader::companions::not_downloaded` lists the files
  opening would read that are dataless; `stat` reports the flag
  (`SF_DATALESS`) without starting a download (82 ms cold, 0.4 ms warm on
  the test database; `cbh-reader/examples/cloud.rs`). Before opening, the frontend
  asks; **Download and Open** requests each file with `brctl download` (or
  a one-byte read) and waits, reporting finished files and bytes in the
  status bar, until all are local; **Stop** ends the wait.
* **Tests.** A reader test and an app test check that a database on disk
  needs no download and that the download returns at once. Frontend tests
  cover Download and Open, declining, and a stopped download. Dataless
  files cannot be made in a test; the detection was checked on the test database.

## Generate Repertoire and the Repertoire Report (2026-10-18)

* **Tests.**
  * The reader's unit tests cover the name split, the marked move's
    parity, merging two games of one code with a single mark, and splitting
    a code of 18 games by table entry.
  * A 2CBH round trip keeps critical marks of kind 1 and 2.
  * An app test generates both databases from a scratch 2CBH, reopens the
    White one (one game, ECO B90, names from the table, 6.Be3 marked),
    refuses to overwrite, and reports three games at the marked position,
    the strongest first, and lists them.
  * Frontend tests cover the dialog flow and the notation's critical marks.
* **The test database.** Release build, `src-tauri/examples/generate_repertoire.rs`,
  the classic test database read-only and warm, player A (6,925 games):
  * Generating took 1.14 s: 468 White and 462 Black repertoire games.
  * The reports took 25–38 s each over the latest 2,000,000 games.
  * The scratch output was deleted afterwards.
* **Two corrections from that run:**
  * Set-up games were replayed from the standard start, which put 86 lines
    under wrong codes. `uci_lines` now keeps standard starts only.
  * Taking the first marked position in repertoire order, as the reference
    app does, left 32 of 400 White topics with games. Taking the deepest
    gives 400.

## Centipawn Analysis (2026-10-18)

* **Tests.** Unit tests cover the time rule, the classifier and the summary.
  An app test runs `mock-uci` on a Ruy Lopez with six moves after the table's
  line. It checks:
  * the opening skipped;
  * a loss of 34 cp and an inaccuracy for every counted move;
  * the summary before the first move, applied as one edit;
  * a symbol at the start refused;
  * cancelling, the seconds bound, and a game with no moves after its
    opening.

  A frontend test covers the dialog, the time per position, the edit and
  the tables.
* **The test database.** Stockfish 17 at Seconds 1 analysed three games of
  the classic test database read-only in 39–50 s each (about 0.77 s per
  position). Average losses were 8–63 cp per side.

## Training annotations (2026-10-18)

* **Tests.**
  * Reader tests build a classic record and a 2CBA entry from the traced
    layout. A 2CBA entry no longer stops parsing; an unknown kind still
    does.
  * A 2CBH round trip keeps a multiple-choice question with an arrow in its
    hint.
  * Unit tests cover the 0-based squares and `set_training`: SAN and UCI
    answers, an illegal answer, the start, and removal.
  * An app test edits a question, saves it to a 2CBH and reads it back,
    checks the PGN export's refusal, clears it and undoes the clearing.
  * A frontend test adds, reopens and deletes a question.
* **The test database.** `training_check` over the classic test database's
  2,304 games with training:
  * all 3,122 questions read, and all 3,687 move answers are legal with
    0-based squares (26 with 1-based);
  * every game round-trips through 2CBA;
  * the game also in the 2CBH sample agrees.



## Material search and result combinations (2026-10-20)

Implemented from latest fetched main in an isolated feature worktree. No source
database, reference executable or extracted proprietary assets were changed.
Reference executable controls and Ghidra addresses, uncertainties and independent
rules are in [search](search.md#material-search-2026-10-20).

Synthetic reader tests cover inclusive piece/minor/total ranges, signed balance,
colour reversal, promotions, setup roots, move windows, duration with fixed
orientation, mainline versus variations, invalid queries, sparse/empty selections,
limits, node IDs and cancellation. Real synthetic 2CBH registry tests cover
intersection/union, duplicate entry identities, entry and column order, retained
and new nodes, empty versus absent results, error/cancel/reset/supersession/closure,
and a deterministic overlapping booster/search cancellation race. A bounded
synthetic union checks retention of existing entries and earliest unique new IDs
with aligned nodes. UI regressions cover retained fields, targets, disabled
unavailable scopes, range validation, reset, independent Position submissions,
progress and cancellation. The development preview also implements set operations.

Independent read-only review approved the core implementation after three
reproduced regressions were fixed. A preview pagination continuation issue was
also reproduced and fixed. Browser UI verification covered All material search,
Within refinement, Add position matches, repeated Add and ascending entry IDs.

Native macOS verification used a disposable five-game synthetic 2CBH and real
Tauri/Rust code: All rook endings with balance 0 → [1,3,5]; Within White Capture
→ [3]; Add pawn endings → [2,3,4]; repeated Add unchanged. All balance ≥+5
returned [3,4], and entry 3 opened at 1.Rxh1/node 1. Native screenshots were
inspected and are linked from [search](search.md#search-ui-evidence). They show
OpenFile with synthetic games, not the proprietary reference application.

Native large-database verification used the classic source read-only. An
unmatched synthetic player prefix showed progress (observed 14%) and returned
an empty result. Add with material balance ≥+144 showed Searching… 0% and Cancel;
clicking Cancel returned Search cancelled and retained 0 of 11,151,119 entries.
Within on that empty result completed with zero hits. No screenshot or private
game data from that database was added to the repository.

Release timing: classic database 11,151,119 index entries, year ≥2024 and one
rook each/no queens or minors: 46,642 hits, 31.58 s first run / 31.90 s repeated
warm run, approximately 7.3 MB peak RSS. Refinement of 3,837 White-Carlsen hits:
499 material matches, 0.162 s excluding initial-result creation. New format,
24,783 entries, material balance ≥+5: 11,294 matches in 0.167 s. No broad
unfiltered full-replay timing is claimed; expensive scans retain at most 100,000
matches, while combined results cap at 2,000,000 and retain prior entries.

Final local checks: **306 Rust tests** and **188 frontend tests** pass; Cargo
formatting, strict workspace/all-target Clippy, TypeScript, Prettier, production
frontend build and native debug/release executable builds pass. The frontend build retains its
existing >500 kB chunk-size advisory. Distribution audit passes for all 641
tracked source/artifact paths. PR checks provide the cross-platform CI record.

The first PR macOS CI run exposed a pre-existing review-practice drag-test race:
the square query could resolve before the asynchronously mounted board finished
its effects. The failure reproduced locally on attempt 4. Awaiting the initial
render and position effects in React `act` passed 20 consecutive targeted runs;
the drag and grading assertions remain unchanged. This is a test synchronization
fix, with no change to board behavior.


## Initial descriptive playing-style report (2026-10-21; superseded)

Implemented from fetched `origin/main` in an isolated worktree. Original checkout,
reference database and unrelated REA sessions/artifacts were preserved. Fresh
REA/Ghidra analysis traced command dispatch, dynamic options, selection, theme
math, reference normalization, internal evaluation, rendering and cancellation.
Addresses, executable hash, directly established behavior and unresolved questions
are recorded in [playing style](playing-style.md). OpenFile independently computes
ten transparent move/material measures, without a UCI engine or comparison grades.

Synthetic tests cover both colours; explicit alias unions; filter-before-limit
selection beyond 100,000 matches; deleted/unknown records; empty/small samples;
checks, captures/en passant, castling, queen movement/exchanges/promotions,
non-pawn inventories, sustained deficits and endgames; setup/unsupported/partial/
corrupt records; replay caps; cancellation, supersession, closure/edit invalidation,
source mutation and supporting-node navigation. UI tests cover criteria, aliases,
measured zero versus missing evidence, complete analysed-game lists, errors,
late callbacks and source-bound navigation. Independent focused review identified
two races that were reproduced and fixed: late evidence validation affecting a
newer report, and source mutation between validation and game loading. Both have
regressions; loading now validates before and after and closes a stale document.

Native acceptance used read-only byte-identical clones of the available Mega
Database 2025 (11,151,119 entries) in a separately identified Tauri app. Exact
identities Carlsen, Magnus #65470 and Nakamura, Hikaru #286135 were checked against
the name list and neighbouring entries; no aliases were added. Both reports used
both colours, no year/rating filter, latest 500 games and a 400-ply cap. Carlsen:
500 analysed, 252 White/248 Black, 2024.05.14–2024.10.01. Nakamura: 498 analysed,
250 White/248 Black, 2024.06.13–2024.09.24, two unreadable records skipped. Samples
include mixed time controls. No career-wide or long-game-only claim is made.

A separate python-chess 1.10.0 calculation matched all ten metric fractions,
colour splits and every evidence SAN/ply in both reports. Native supporting links
opened Carlsen–Firouzja at 45...Qxg7 (ply 90, nine non-pawn points each), and
Nakamura–Naroditsky at 8.Kxd1 (ply 15, no queens), with the expected board and
highlighted move. Native Stop during a header scan was also checked: a WebKit
form-default restart was reproduced, fixed with a prevented click default, and
verified in the rebuilt app with a regression test. The stopped state persisted
without publication; a subsequent explicit Generate returned the expected sample.
The original screenshots were superseded by the native-dimension replacement
below; these historical measurements do not validate the replacement.

Warm-cache release selection/replay totals were 0.369 s / 0.285 s, with full-index
scans 0.172 s / 0.131 s and replay 0.197 s / 0.154 s, respectively. Prepared
name-list lookup took 0.005 s / 0.002 s. Whole example processes including PGN
validation export peaked at 12.4 / 12.3 MiB resident memory. Native debug UI
computations took 3.04 s / 2.96 s. Engine cost: zero. These measurements exclude
initial database opening/name-index construction; cloud placeholders can stall
filesystem reads. Selection retains one 8,192-header chunk plus at most 2,000
candidates. No result cache is persisted. Nine original database-family file
signatures remained unchanged, and original/clone CBH hashes agree.

Final local checks: **329 Rust tests** (23 new style tests), **195 frontend tests**
(7 new style tests), Rust formatting, strict workspace/all-target Clippy,
TypeScript, Prettier, production frontend build and locked native release build.
The native acceptance app also builds successfully with embedded frontend assets.
The frontend retains its existing >500 kB bundle advisory. The tracked-source
allowlist permits the authored style documentation/plans and screenshots, without
raw decompilation, private games or databases. Cross-platform CI is recorded by
the unmerged PR.


## Native-dimension replacement (2026-10-22)

The initial ten-metric report is superseded by the eight ChessBase dimensions.
Fresh REA/Ghidra evidence covers 82 functions and 17 batches; the categorical-grade
initialization defect was confirmed in assembly. Score and Decided Games use
recovered formulas, result denominators, references and normalization. Tenacity
exposes verified draw inputs, but its complete score and the other five composite
scores remain unavailable. All categorical grades remain unavailable. The
checklist deliberately marks native coverage partial; no displayed Windows
ChessBase comparison or full native compatibility is claimed. Full evidence,
formulas and dependencies are in [playing style](playing-style.md).

Reader fixtures verify classic and 2LID tournament metadata, record bounds,
legacy stream remapping, Simultaneous/Blitz/Rapid/Long precedence, unknown category
policy and native minimum stored length before the bounded sample. Runtime tests
cover both colours, canonical forfeits, unknown denominators, measured zero,
missing colours, all eight dimensions, absent grades/proxy scores, Tenacity draw
inputs, source validity and evidence. UI tests retain cancellation/supersession,
source-bound navigation and stale-callback guards and cover game-category filters.
A deterministic regression cancels within a 1,024-record header batch at 256 rows;
cancellation is now checked before each metadata lookup. Independent review
confirmed both findings fixed and no residual actionable concerns.

Actual rebuilt native UI acceptance used read-only byte-identical Mega Database
2025 clones (11,151,119 entries), exact Carlsen #65470/Nakamura #286135 identities,
no aliases, both colours, Long only, no year/rating bounds and latest 500 eligible
games. Carlsen: 492 analysed, 247 White/245 Black, 2016.09.12–2024.09.21, eight
setups skipped; Score 77.1442%, Decided Games 35.5805%. Nakamura: 483 analysed,
246 White/237 Black, 2015.10.19–2024.06.07, seventeen setups skipped; Score
67.2984%, Decided Games 15.7021%. These normalized composites are neither game
fractions nor player percentiles. The other six score cards remain unavailable.

Independent raw CBH/CBT reads reproduced selection, eligible counts and category
filtering; separate python-chess 1.10.0 replay matched result totals, both raw and
normalized calculations, draw inputs and every evidence SAN/ply. Native links
opened Carlsen–Schitco at 42.g5 (entry 11131908, ply 83), and
Praggnanandhaa–Nakamura at 31.Rxe8+ (entry 10932339, ply 61), with the expected
board and highlighted move. Native Stop during scanning remained stopped without
publication. [Inspected screenshots](playing-style.md#native-ui-evidence) retain
both real reports, inputs, missing dimensions, supporting positions and Stop.

Warm-cache release totals were 0.408/0.351 s: header selection 0.189/0.159 s,
replay 0.218/0.191 s; prepared name lookup 0.003/0.002 s. Whole validation/export
processes peaked at 16.5/15.9 MiB RSS. An initial name-index build took 0.461 s and
peaked at 71.3 MiB process RSS. Database opening is excluded; engine cost is zero.
Native debug UI computations took 3.14/3.32 s.
All nine original source signatures stayed unchanged; original/clone CBH hashes
match. No private database, sample PGN or proprietary asset is distributed.

Final local checks: **342 Rust tests**, **196 frontend tests**, Cargo formatting,
strict workspace/all-target Clippy, TypeScript, Prettier, production frontend
build, locked native release build and embedded-frontend native acceptance build.
The existing bundle-size advisory remains. The unmerged PR records cross-platform
CI. The tracked-source distribution audit passes for all 661 tracked paths.

## Engine search commands (2026-10-22)

* **Engine crate.** A test sends `go infinite searchmoves …` to the mock
  engine, which then limits its lines to those moves. A search move that is
  illegal in the position is refused before anything is sent.
* **App tests.** Analyse threat's null move gives the other side the move and
  drops the en passant square; a side in check is refused. Next Best leaves
  18 of the 20 opening moves after two are excluded, ignores an unreadable
  move, and refuses to exclude every move.
* **Frontend tests:**
  * Y, Shift+Y and X restart the kibitzer with the right search mode.
  * Search Better reports whether the new best line beats the move left out.
  * A move on the board clears the moves left out.
  * Lock Engine keeps the engine on its position; releasing the lock follows
    the board.
* **Real engine.** Stockfish 17 with `position startpos moves e2e4` and
  `go depth 14 searchmoves g8f6 b8c6` chose b8c6, one of the allowed moves.
  A threat position (a null move with White to move again, the queen on h5
  and the bishop on c4) gave Qxf7 mate as its best move.

## Evaluation profile and instant analysis (2026-10-22)

* **Format.** Read-only and warm, with a probe through the reader's `load`:
  * the classic and 2CBH families of the test database decode the same 177
    profiles for entries 1–24,783, digest for digest. Every profile sits at
    the game's start, and every 2CBA sub-block is exactly 2 + 4n bytes long;
  * 121 profiles have one entry per main-line ply plus the start. 48 are
    shorter and 8 longer;
  * mates are positive in 110 of the mate entries of 1–0 games, and negative
    in 80 of the mate entries of 0–1 games, so the scores are from White's
    side;
  * classic entries 11,000,001–11,100,000 hold 210 profiles, all decoded and
    none reported as unsupported, read in 20.5 s.
* **Tests.** Reader tests cover both layouts, the count limit, a wrong length
  and an absent profile. The 2CBH writer round-trips a profile. App tests run
  instant analysis on the mock engine: every ply is filled, a second run comes
  from the cache, and a cancelled run evaluates nothing. Frontend tests cover
  the critical-move rule, the instant analysis parameters, progress, Stop,
  jumping to a critical move, and the profile above the notation.
* **Real engine.** Release build, Stockfish 17, level 3 (105 ms per
  position), the classic test database read-only:

  | Game | Positions | Fresh | From the cache | Centipawn Analysis at 0 s |
  |---|---|---|---|---|
  | entry 144 | 92 | 11.1 s | 0.20 s | 19.7 s |
  | entry 731 | 58 | 6.9 s | 0.22 s | 8.3 s |
  | entry 11,000,000 | 74 | 8.5 s | 0.21 s | 13.6 s |

  That is about 0.12 s per position against the 105 ms asked. The repeat
  costs the engine's start only. Centipawn Analysis asks 200 ms per position,
  more than the cache holds, so it evaluates afresh.

## Centipawn Analysis of a database list (2026-10-22)

* **Unit tests:**
  * the performance formula: 75 % against 2400 and 2600 gives 2691; 100 %
    and 0 % give ±800; unrated opponents and unknown results are left out;
  * the statistics' map (names with commas made spaces, each player's first
    rating), the weighted average, the row and game orders, and the CSV's
    header and row layout.
* **App tests** use a scratch 2CBH and the mock engine, which loses 34 cp
  each move:
  * two games are analysed and replaced, and a short game is skipped;
  * each player's values are 34, and Points and Perform agree with the hand
    calculation (2591);
  * the replaced entry holds the summary text and an evaluation profile;
  * a run stopped at once writes nothing;
  * a PGN collection is refused, since only a 2CBH opened for editing can be
    written.
* **Frontend test:** the dialog sends the selection and parameters, shows the
  players, and saves the CSV to the chosen path under the default name.
* **Real engine:** 20 classic test-database games copied to a scratch 2CBH
  (deleted after), Stockfish 17 at Seconds 0, release build. 19 games were
  replaced in 274 s and one, with no moves after its opening, was skipped.
  The CSV had 37 players.

## Engine analysis history (2026-10-22)

* The protocol test reads UCI `time` in milliseconds and refuses a malformed
  value.
* A frontend test sends four lines for one position (e4 at depths 10 and 12,
  then d4 at 14 and 15). History shows two rows, latest first: d4 chosen at
  2.5 s with depth 15 and +0.28, then e4 at 0.1 s with depth 12 and +0.25.
  Selecting another move empties it.
* Stockfish 17 sends `time` in its info lines (for example `info depth 14 …
  time 74 pv …`), so real rows carry the engine's own times.

## Move medals, times, diagrams, pawn structures, piece paths, evaluations (2026-10-22)

* **Format.** Read-only and warm, through the reader's `load`:
  * Entries 1–24,783 of both families give the same extras on the same moves
    (63 games, 67 moves in 2CBH). The classic file has two more medals, on
    entries 9014 and 20115. In their 2CBH copies a game quotation (0x13),
    which the 2CBA reader cannot size yet, stops the reading earlier.
  * Classic 11,000,001–11,150,000: 8,040 move times in 100 games, read in
    36.7 s. Every hour, minute and second byte is below 60, and 23 have a
    non-zero fourth byte. These games stay Partial for types 0x16, 0x17 and
    0x24, which are not decoded.
  * Classic 3,000,001–3,100,000: 972 annotated moves in 557 games, read in
    23.9 s. All 244 pawn structures are 3. All 112 piece paths are the
    colour code 3 and the square their move lands on.
  * A classic slice from 8,000,001 holds 6 stored evaluations, with
    score, kind and depth (e.g. 0, 0, 46).
* **Tests:**
  * a reader test for all six types in the classic layout, then through the
    2CBH writer and back;
  * chess-core tests: setting and clearing each mark, refusing a time or
    path at the start and an invalid time, and a PGN round trip of
    `[%emt]` and `[%eval]` (including a mate);
  * Delete Commentary: "text" keeps them; "evaluation and time" removes
    them;
  * frontend tests: medal names, time packing, the notation marks, the
    board's pawn squares and path arrows, and the Set Medal, Move Time and
    Insert Diagram edits.

## Find Novelty (2026-10-22)

* App test on a scratch 2CBH (two reference games): a new line's novelty at
  ply 4 after 5 questions, with both games listed. A game of the database
  finds its novelty at ply 5, its own entry left out. A line other games
  cover has none, and a stopped search finds nothing.
* Frontend tests: Shift+F6 sends the document and database, shows the
  novelty, the count and the first games, selects the novelty, and reports a
  database that cannot be searched move by move.
* Classic test database with its opening tree, release build, read-only,
  warm: 200 random entries in 19.3 s (about 23 questions a game). A halving
  search would have given the same novelty for 193 of the 200; the other 7
  return to the known games by a transposition.
