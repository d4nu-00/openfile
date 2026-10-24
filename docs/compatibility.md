# Database compatibility

The reader supports two families, both opened read-only. Text entries are not
games in either family.

* **Classic:** 46-byte CBH headers and game entries, optional CBP player, CBT
  tournament, CBC annotator, CBS source and CBE team names (team references
  from CBJ), and standard classical CBG move streams.
* **Newer 2CBH:** 192-byte records, framed 2CBG move trees (standard and set-up
  starts) and 2LID player, tournament, source and team names. Annotators are
  player entries. See [formats](formats.md).

CBG records with no flags or only `0x40000000` (28-byte setup) are
supported. So are:
* Chess960 records (`0x0a000000`, with a set-up);
* the special encodings `0x04000000`, `0x01000000` and `0x05000000`
  (single lines);

see [formats](formats.md#classic-cbg-encoding-kinds-and-chess960).

In the classic test database, all 892 Chess960 games with a set-up decode (77,149
moves, variations included), and so do all 11 special records. The one
Chess960 record without a start position is refused. The following return
an unsupported status without a partial game:
* unencoded records (`0x80000000`);
* other encoding kinds;
* two-byte-word records with variations.

Chess960 games read and write PGN with `[Variant "Chess960"]` and a FEN whose
castling letters name the rooks. Castling is stored king to rook, as UCI
writes it for Chess960. They cannot be saved to 2CBH yet: 2CBG's encoding of
Chess960 was not traced, and the 2CBH sample has no Chess960 game to check
it against. Saving one is refused, and Paste leaves them out and counts
them.
Setup control byte 0 must be 0 or 1. Every the test database setup stores 1,
and cbh2pgn and libcbh both skip the byte. Byte 1's upper three bits and byte
2's upper four bits must be zero; other values return `unsupportedSetup`. The turn bit is `0x10`,
en-passant file is the low four bits (0 absent, 1–8 a–h), and castling rights
use low bits 1/Q, 2/K, 4/q, 8/k. Move number 0 is read as 1. An en-passant file that is impossible on the board
is dropped, matching the reference app's own 2CBG copy of the same game. A castling
right whose king or rook is off its standard square is dropped too, one right
at a time; the rest are kept. Twelve set-ups of the classic test database need this
(entries 64687, 64740, 68207, 96531, 96535 and seven others). They are games
taken up in the middle, with the kings already castled or moved but K and k
still set, and all twelve then decode to their last move. A set-up is always
tried as stored first. A standard record is read as standard chess before
Chess960, so a stored right is never taken for a Chess960 one; no game of the
classic test database relied on that. How the reference app itself treats such rights was not
traced: its copies of these games are outside the 2CBH sample. 2CBG set-ups
follow the same rule. The occupancy stream is file-major and MSB-first, with
standard chess position validation.

En-passant controls were corroborated against
[libcbh PositionStack::setup](https://github.com/rolandlo/libcbh/blob/main/src/cbh_position.cpp).
This corrects the three-bit mask in the older cbh2pgn reference.

Ordered variations, null moves outside check, filler bytes and modulo-256 move
counters are supported. A terminal root marker is required, with all branch
frames closed. Byte, node and variation-depth limits apply to every import.

Original CBA and 2CBA annotations are decoded into the editable game tree:
* text after and before moves;
* move, position and prefix symbols as NAGs;
* coloured squares and arrows.

Other annotation types are not shown. These include training, game
quotations, medals, critical positions, pawn structures, piece paths,
evaluation profiles, variation colours and multimedia. Games that contain them
load with status `partial` and a diagnostic, and exporting them needs the same
acknowledgement as before.

Only one language of translated text is shown per game, plus text with no
language (see [formats](formats.md)). A malformed annotation record leaves the
moves loaded and reports the annotations as `corrupt`.

PGN import and export carry text before moves and `[%csl]`/`[%cal]` marks.

Classic name tables (players, tournaments, annotators, sources, teams) are
read as UTF-8 when valid, otherwise as Windows-1252; the bytes 0xA2–0xA6,
pieces in the reference app's figurine fonts, show as ♔ ♕ ♘ ♗ ♖ (see
[validation](validation.md#names-with-figurines-2026-10-13)).
Multimedia and proprietary cloud services are not supported.

Sources are checked by path (identity, size and time) when an operation
starts and at most every 200 ms while it reads. A file changed in place in
between is noticed by the next check (see [search](search.md)).

In the newer family, the following are refused or not read:
* 2CBG text blocks and extended (for example Chess960) move tokens;
* set-up controls other than those observed;
* blocks whose marker, trailer or checksum fails. These are reported corrupt,
  never partially loaded.

Title lists (2LID) and the other CBJ fields are not read. Result codes 3–7 are
shown as the reference app shows them: lines with their evaluation symbol or "Line",
and forfeits as `+:-`, `-:+`, `=:=` and `0-0`. PGN export keeps a forfeit's
winner and writes `*` for lines and `0-0`. Codes above 7 are reported as
uninterpreted.

**Packed ECO** is decoded in both families. Bits 7–15 hold 1–500 for A00–E99,
and the reference app sub-classification in the low seven bits is not shown. The
formula was verified against the opening moves of 24,162 games.

Local validation on 2026-10-11 compared 238 unique selected entries against the
pinned MIT cbh2pgn reference: 235 supported games matched ordered UCI trees,
initial positions, dates and results; three text entries were explicitly
unsupported. Selection combined the 39 retained overlap records (36 games,
three texts) and 200 deterministic spread ordinals. No comparison failures
occurred. Once cloud placeholders were available locally, the largest measured
page and game load took 4.3 ms and 4.2 ms; total native bytes read across the
comparison was 119,946. Peak child RSS was 7.1 MiB. The final rerun matched the
same 235 games with zero failures, with 5.62 ms page and 349.00 ms load maxima
(including filesystem/cache variability). These samples establish
bounded compatibility evidence, not full database or annotation coverage.
Cloud-backed files may take substantially longer on first access.

Classic CBH is read-only. 2CBH databases can be created, or opened for editing
with an explicit command. Games can then be saved, replaced and marked
deleted; see [writing](writing.md):
* Moves, game data, players, tournaments, annotators and sources are written.
* So are text before and after moves, symbols, squares and arrows.
* Teams, the reference app's index files (`.2lcd`, `.2lgd`) and several list fields
  (hash, timestamps, material) are not written.
* Saving a game whose original annotations are not decoded, or that holds
  content the reference app cannot store, needs the same acknowledgement as PGN
  export.

PGN saving writes an independent file and requires that acknowledgement when
original annotations are omitted. PGN supports legal
standard chess moves, setup FEN, ordered variations, comments and numeric NAGs;
imports are bounded to 20 MiB/2,000 games/100,000 aggregate nodes by default.
Missing companions, corrupt records and unsupported records are reported
separately; an unsupported game is not silently flattened into a partial game.

Centipawn Analysis of a database list writes the reference app's per-player
CSV columns; its game weights come from the workbench's move weights rather
than the reference app's engine complexity, and Weighted Av is printed
unscaled (see [centipawn analysis](centipawn-analysis.md)).

Move medals, move times, diagrams, pawn structures, piece paths and stored
evaluations (annotation types 0x22, 0x07, 0x06, 0x14, 0x15, 0x21) are read
from both families, shown, and written back to 2CBH. Move times and
evaluations travel through PGN as `[%emt]` and `[%eval]`; the others have
no PGN form. The move time's fourth byte (23 of 8,040 in a sample) is kept
but not shown. Types 0x16, 0x17 and 0x24, common in recent games with move
times, are still reported as unsupported (see [annotations](annotations.md)).

Evaluation profiles (annotation type 0x26) are read from both families and
written back to 2CBH unchanged. Scores and mates are shown. About 0.2 % of
recent test-database games carry one. Kind 32 (5 entries in 177 profiles) is
kept but not shown (see [evaluation profile](evaluation-profile.md)). Instant Analysis
uses the reference app's time per position; its own profile is shown but not
stored in the game.

Local UCI engine analysis covers standard chess and Chess960. A Chess960
position is sent with `UCI_Chess960` set to true, and only to engines that
offer that option. Other variant rules cannot be enabled. Deep Analysis
follows the reference app's traced schedule with one engine, and the analysis jobs
queue replaces games of a 2CBH database opened for editing with their
analysis; the reference app's jobs files are not read or written (see
[analysis](analysis.md)). ECO codes come from the open lichess opening table
(CC0), not from the reference app's `Eco.cod`, and agree with the reference app's own codes
for about 69–76 % of the test database's games; Set ECO Codes keeps existing codes by
default and never writes a sub-classification (see [openings](openings.md)).
The opening report ports the reference app's history, players, moves and main line,
but not its plans, recommendations or opening keys. Prepare against White or Black
shows the player's games of one colour, their repertoire lines and their
tree in the Book pane; the reference app's game window mode for them and its
rating formula were not traced (see [preparation](preparation.md)). The
player dossier ports the reference app's statistics, verdict bands, years,
opponents, tournaments and repertoire; medal games stand in for its
"spectacular games", mates are found by replaying decisive games, and the
biography, pictures and rating history of its player encyclopedia are left
out (see [player dossier](player-dossier.md)). The tournament report lists the
crosstable's games round by round with the reference app's round range and two
columns; its HTML page, German headings and per-round PGN files are not
ported (see [tournament report](tournament-report.md)). The Error Report analyses
the selected games with the user's UCI engine to the reference app's depths and
applies the reference app's Blunder Elo curve to an error value formed as
The reference app forms it, but without its engine's complexity and deep-search
weights; the estimate is approximate (see [error report](error-report.md)). Generate Repertoire writes
White and Black repertoire databases as new 2CBH files from the open
database, with the reference app's merge and marking rules; its clustering
is replaced by the ECO table's entries, and the Repertoire Report takes each
game's deepest marked position (see [repertoire](repertoire.md)). Critical
position annotations (0x18) are read, written and shown. Centipawn
Analysis applies the Error Report's pass to the shown game after its
opening, with the reference app's categories where a UCI engine allows them
(see [centipawn analysis](centipawn-analysis.md)). Training questions (0x09)
are read from CBA and 2CBA, written to 2CBA, shown and edited (see
[training](training.md)). The search mask's Annotations
and Medals tabs search texts, symbols, colours, training, multimedia, pawn
structures, piece paths, critical positions, variations and medals; how
The reference app combines several symbols or medals was not traced (see
[search](search.md)). Engines and private databases are
separate user-provided files. The first workbench does not yet implement
The reference app's full search/indexing, its own opening book files (.ctg and trees),
training, multimedia, cloud services or lossless writing of every annotation type of the
reference app. The workbench's opening tree and reference search are built
from the user's own databases into the app's data folder (see
[opening tree](opening-tree.md)). They count main lines only, store positions
reached by at least five games within 60 half-moves, and may miss a game that
transposes into a rarer position. The game list scrolls the whole database
and sorts by any column, from sort orders kept in the same folder (see
[game list](game-list.md)). It sorts names by its own collation: case is
folded and common accents removed. The reference app's own collation was not traced.
Its Fix Sort Order and Sort Database, which rewrite the files, are not
implemented.

Any database can be converted into a new .2cbh database, whole or a
selection of its games (see [conversion](conversion.md)). As with Paste,
annotations that are not decoded or not storable are left out and counted.
Text entries and Chess960 games are left out too, and deleted marks are
kept. The source is only read.

Check Integrity reads any database and reports what cannot be read; it
repairs nothing. In a .2cbh database opened for editing, Find Double Games
marks doubles deleted, and Remove Deleted Games rewrites the files without
deleted entries, copying the other entries' data byte for byte. It removes
The reference app's `.2lcd`/`.2lgd` indexes for the reference app to rebuild (see
[maintenance](maintenance.md)).

Games can be copied from any open database and pasted into a .2cbh database
opened for editing. Annotations that are not decoded, or that the reference app
cannot store, are left out and counted, as when a single game is saved. PGN
cannot express forfeits or "0-0". Loaded games keep such results in a
`CBResult` tag, which the writer stores again and PGN exports carry. macOS native operation is verified locally;
Windows/Linux native validation remains outstanding. See [validation](validation.md).


## Material and result combinations (2026-10-20)

Both classic and new-format databases support material replay queries and
All/Within/Add search targets. Material uses independently implemented count
ranges and signed Q9/R5/B3/N3/P1 balance, including promotions, setup roots and
optional variations. ChessBase's signed per-piece Difference mode, pawn
structure/bishop-quality options and example catalogue are not implemented.
Reference evidence, uncertain native rules, hit limits and the separate
Position-tab behavior are recorded in [search](search.md#material-search-2026-10-20).
No source database or native reference index is written by these searches.
