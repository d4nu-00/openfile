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

In the classic Mega, all 892 Chess960 games with a set-up decode (77,149
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
Setup control byte 0 must be 0 or 1. Every Mega Database 2025 setup stores 1,
and cbh2pgn and libcbh both skip the byte. Byte 1's upper three bits and byte
2's upper four bits must be zero; other values return `unsupportedSetup`. The turn bit is `0x10`,
en-passant file is the low four bits (0 absent, 1–8 a–h), and castling rights
use low bits 1/Q, 2/K, 4/q, 8/k. Move number 0 is read as 1. An en-passant file that is impossible on the board
is dropped, matching ChessBase's own 2CBG copy of the same game. The occupancy stream
is file-major and MSB-first, with standard chess position validation.

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
pieces in ChessBase's figurine fonts, show as ♔ ♕ ♘ ♗ ♖ (see
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
shown as ChessBase shows them: lines with their evaluation symbol or "Line",
and forfeits as `+:-`, `-:+`, `=:=` and `0-0`. PGN export keeps a forfeit's
winner and writes `*` for lines and `0-0`. Codes above 7 are reported as
uninterpreted.

**Packed ECO** is decoded in both families. Bits 7–15 hold 1–500 for A00–E99,
and the ChessBase sub-classification in the low seven bits is not shown. The
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
* Teams, ChessBase's index files (`.2lcd`, `.2lgd`) and several list fields
  (hash, timestamps, material) are not written.
* Saving a game whose original annotations are not decoded, or that holds
  content ChessBase cannot store, needs the same acknowledgement as PGN
  export.

PGN saving writes an independent file and requires that acknowledgement when
original annotations are omitted. PGN supports legal
standard chess moves, setup FEN, ordered variations, comments and numeric NAGs;
imports are bounded to 20 MiB/2,000 games/100,000 aggregate nodes by default.
Missing companions, corrupt records and unsupported records are reported
separately; an unsupported game is not silently flattened into a partial game.

Local UCI engine analysis covers standard chess and Chess960. A Chess960
position is sent with `UCI_Chess960` set to true, and only to engines that
offer that option. Other variant rules cannot be enabled. Engines and private databases are
separate user-provided files. The first workbench does not yet implement
ChessBase's full search/indexing, its own opening book files (.ctg and trees),
training, multimedia, cloud services or lossless writing of every ChessBase
annotation type. The workbench's opening tree and reference search are built
from the user's own databases into the app's data folder (see
[opening tree](opening-tree.md)). They count main lines only, store positions
reached by at least five games within 60 half-moves, and may miss a game that
transposes into a rarer position. The game list scrolls the whole database
and sorts by any column, from sort orders kept in the same folder (see
[game list](game-list.md)). It sorts names by its own collation: case is
folded and common accents removed. ChessBase's own collation was not traced.
Its Fix Sort Order and Sort Database, which rewrite the files, are not
implemented.

Games can be copied from any open database and pasted into a .2cbh database
opened for editing. Annotations that are not decoded, or that ChessBase
cannot store, are left out and counted, as when a single game is saved. PGN
cannot express forfeits or "0-0". Loaded games keep such results in a
`CBResult` tag, which the writer stores again and PGN exports carry. macOS native operation is verified locally;
Windows/Linux native validation remains outstanding. See [validation](validation.md).
