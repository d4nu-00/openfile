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

The `sort` example built sort orders on the Mega sample (release build, warm
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

The `names` example on the classic Mega (release, warm) listed 463,258
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
* 2,000 games of the classic Mega in 1.0 s;
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

`src-tauri/examples/open_timing.rs` timed opening games of the classic Mega
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
* App tests cover the settings file: ChessBase's default of 8 tabs, checks
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

A debug build on the classic Mega opened entry 27878 at start, then ran
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

A debug build on the classic Mega ran Default Kibitzer with Stockfish 17
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

Of the classic Mega's 463,262 players, 362 name fields are not ASCII and
some were not valid UTF-8; their bytes were 0xA4 (323), 0xA5 (289), 0xA3
(111), 0xA6 (43), 0xA2 (10), 0xA0 (3) and 0xAA (1). The 0xA2–0xA6 names are
opening lines ("1.b3 e5 2.?b2 ?c6 3.e3", "10...?b8"), where the bytes are
the pieces K, Q, N, B, R of ChessBase's figurine fonts: "?b2" after b3 e5 is
Bb2, "?c6" Nc6, "ed5 ?d5" Qd5, "?fd8" Rfd8, "?b1" Kb1. 0xA0 occurs in real
names ("Rodrigo Pereira") as Windows-1252's no-break space. Classic names are
now read as UTF-8 when valid, otherwise as Windows-1252 with 0xA2–0xA6 shown
as ♔ ♕ ♘ ♗ ♖; they no longer show "�". The meaning of 0xAA (once, in an
opening line) is not known; it shows as "ª".

## Crosstables through a sort order (2026-10-14)

`cbh-reader/examples/crosstable.rs` timed crosstables of the classic Mega
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
Mega (release, warm). Carlsen: 6,925 games found in 0.056–0.066 s by 8
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

* **Parallel scans.** On the classic Mega the parallel game-data search, Find
  Position and booster build returned the same results as before. The
  Ruy Lopez search found the same first hits (entries 231, 410, 412, 476,
  730, each at node 5). The Opera Game position gave the same 1,035 hits
  with and without the booster.
* **Source reads.** The first parallel Find Position took 651 s for the
  Ruy Lopez. `time` showed more system time than user time, and on the 2CBH
  Mega one worker spent 2.0 s in the kernel against 0.7 s of chess work.
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

A census of the classic Mega's CBG headers counted:
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
  classic Mega.

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
  Mega families, a missing database and the Mega's folder, and the file was
  deleted afterwards. The screen showed 11,151,119 entries (CBH) and 24,783
  (2CBH), read from the header records alone. The first attempt was taken
  before the cold first header reads had finished, so the screen now says
  "Reading the lists…" until they arrive.

## Keyboard symbols and Tactical Analysis symbols (2026-10-14)

* **Evidence.** The symbol popups, their order and the score-to-symbol
  table were read from CBase18 18.1 statically. They are listed in
  [game window](game-window.md#annotation-symbols-from-the-keyboard) and
  [engines](engines.md#tactical-analysis-blunder-check).
* **Mega check.** All 24,759 annotation blocks of the Mega's 2CBA file were
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
  50,000-entry slices of the classic Mega and the whole 2CBH Mega into the
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
  * Check Integrity on the 2CBH Mega found no problems (0.18 s).
  * On the classic Mega it took 133.8 s and found 16 problems in
    11,151,119 entries. 12 are set-ups whose castling rights the board
    does not allow, which the reader refused and earlier spot checks had
    not met. Two are damaged move records. One is the known Chess960
    record without a start position, and one is an annotation record that
    does not parse. (This first listed 14 castling set-ups: the example
    prints only the first five problems, and the two damaged move records
    were counted with them. The 2026-10-15 re-run below lists them all.)
  * Find Double Games marked 4 games of the 2CBH Mega with the defaults.
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

* **Mega check.** The medal word at 2CBH bytes 130–131 equals classic
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
* **Frontend tests.** "+" marks the Shift-selection and reports ChessBase's
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
  * ChessBase's schedule: 4 s, depth 2 and widths 3/1 at iteration 0; 7 s,
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
  * ChessBase's minimum times (15 s, 30 s), the 1,000-job bound, Skip,
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
  * The time warning holds ChessBase's minimum.
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
  * the written codes read back, and a ChessBase sub-classification on an
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
* **Measurements** on the Mega, read-only: see [openings](openings.md).
  * 2CBH family: 24,759 games in 0.26 s, the same main code as ChessBase
    for 76.4 % of coded games.
  * Classic: 11.15 M games in 188.5 s with 8 workers, 68.6 %.

## Opening report (2026-10-15)

* **App test** on a new 2CBH database of 62 games:
  * 60 reach 1.e4, with results 10/20/30;
  * history: the earliest (1990) and latest (2019) games, years, and
    5-year fashion buckets against the database's years;
  * players: the 2700 player as Black at 75 %, and ChessBase's notable
    rule picking the two regular players over twenty one-game club players;
  * move rows from the games read (c5 40, e6 20, with "played by" and
    scores), then from the opening tree with performance once one is built;
  * the main line 1...c5 2.Nf3 d6 3.d4;
  * the two 1.d4 games listed individually.
* **Reader test:** years from the reader's dates.
* **Frontend test:** the report shows its title, line, results, history,
  year chart, players, moves with "played by", and the main line numbered
  after the position.
* **Measurements:** see [openings](openings.md#measurements-1). The 2CBH Mega
  report of 1.e4 c5 takes 4.5 s; the classic Mega's Najdorf takes 23.8 s
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
* **Mega check** (read-only): 62 medalled 2CBH records, mostly Endgame
  (22) and Best game (13).

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

## Set-ups with impossible castling rights (2026-10-15)

* **Finding.** Twelve classic Mega set-ups store castling rights that the
  board does not allow (entries 64687, 64740, 68207, 96531, 96535 and seven
  others): games taken up in the middle, mostly from 1934–1946, with K and k
  (bits 2 and 8, or 2 and 4) set while the kings stand on g1/g8 or elsewhere.
  The reader refused them as "invalid castling rights".
* **Change.** A right whose king or rook is off its standard square is
  dropped, one right at a time, after the set-up as stored has been tried
  (`setup_game` in `cbh-reader/src/moves/setup.rs`, shared by classic and
  2CBG set-ups). Standard records are read as standard chess before
  Chess960, so a stored right is never taken for a Chess960 one. A
  temporary counter over the whole classic Mega found no standard record
  that only the Chess960 reading accepted, so no other game changes.
* **Result.** All twelve now decode to their last move with plausible
  play. Check Integrity on the
  classic Mega, release build, 8 workers, warm: 157.9 s with a Ghidra
  analysis running beside it, 4 problems instead of 16, namely two damaged
  move records (3079389, 7168922), the Chess960 record without a start
  position (489834) and the annotation record (2455498).
* **Not traced.** How ChessBase itself treats these rights: its 2CBH copies
  of these games are outside the 24,783-entry 2CBH sample, and its set-up
  validation was not followed.
* **Tests.** `setups.rs` builds set-ups with kings on g1/g8 and K, k set
  (both dropped), and with one possible and one impossible right (the
  possible one kept, and castling with it decodes).

## Prepare against White or Black (2026-10-15)

* **Measurements** on the classic Mega, read-only: see
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
* **Tests.** Reader tests check ChessBase's band tables, the per-colour
  tally (score rounding, performance, game lengths, draws and quick draws,
  forfeits left out) and the score by rating band. An app test builds the
  dossier of a scratch 2CBH player: score bands by colour, quick draws,
  rating bands, a mate given and one received, a medal set in the record,
  a self-annotated game, the White repertoire, and listing the kept games.
  Frontend tests open it with Ctrl+Alt+D, check the workbench's wording
  and that missing values show as nothing, list an opponent's games and
  load a mate.
* **Real app.** A debug snapshot of a 446-game player of the 2CBH Mega
  showed "null%" for missing values; the dialogs (and Player statistic's
  score) now treat null as missing.

## Tournament report (2026-10-16)

* **Measurements** on the classic Mega, read-only: see
  [tournament report](tournament-report.md#measurements).
* **Tests.** An app test reports a scratch 2CBH tournament of three players:
  rounds in order with the unnumbered game last, the earliest date, the
  pairings, the standings after each round (ties in crosstable order), the
  result statistics, and listing a round's games with `show_entries`, which
  refuses entries out of range. Frontend tests open the report from the
  Tournaments list and from the Cross table, apply the round range, trim
  unknown date parts, list a round's games and load a game.
* **Real app.** A debug snapshot of a 17-round match of the 2CBH Mega showed
  dates as "1905.??.??"; unknown parts are now left out.

## Error Report (2026-10-16)

* **Measurements** with Stockfish 17 on the classic Mega, read-only: see
  [error report](error-report.md#measurements). The first version's
  estimates were 400–700 points low; tracing showed ChessBase divides the
  plain losses by weights of at least 2 for ordinary moves, and the
  corrected version came within 82–292 points on three strong players'
  latest games.
* **Tests.** Unit tests check ChessBase's Blunder Elo curve, the error
  value, the bands, severity, distribution, transitions, phase and opponent
  strength. An app test runs `mock-uci legal` over a scratch 2CBH: one
  player's moves or both sides', a game without the player skipped, losses,
  the error value, the rating, too few moves for an estimate, Stop, and an
  empty selection refused. Frontend tests run the dialog with a player and
  with both sides, and load a worst move.

## Annotation and medal search (2026-10-16)

* **Measurements** on the classic Mega, read-only: see
  [search](search.md#annotations-and-medals-tabs-2026-10-16). Each search
  took about 1 s, reading only the annotation records of annotated games.
* **Tests.** A reader test writes a scratch 2CBH with texts, symbols,
  coloured squares and arrows, a variation and a user medal, and checks
  each criterion (two texts, whole words, symbols, any text or symbol,
  colours, variations, medals). Another checks the critical-position kinds,
  training and multimedia on parsed entries. A frontend test opens Search
  text on the Annotations tab and searches with symbols, colours, a
  critical position and two medals.
