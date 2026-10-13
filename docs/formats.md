# Newer ChessBase database format (2CBH family)

This note records the facts behind the `cbh-reader` support for the newer
database family. The reader lives in `crates/cbh-reader/src/v2/` and is chosen
by `Database::open` when the user opens a `.2cbh` file. The classic `.cbh`
family uses the same `Database` type and API.

Evidence comes from two kinds of work. Static analysis of CBase18.exe 18.1
(SHA-256 `8d633b10…8e8dee`) used REA/Ghidra; the program was never run.
Bounded reads of a private Mega Database 2025 copy confirmed the layouts. That
copy holds both families under one name, and every shared entry was compared
(see [validation](validation.md)). Addresses below are virtual addresses in
that executable. No ChessBase code or resources were copied.

## Files

| File | Role | Status |
|---|---|---|
| `.2cbh` | Entry index: one 192-byte record per game or text | Read |
| `.2cbg` | Framed blocks: move trees (type `0x0001`) and texts (`0x1000`) | Moves read; texts unsupported |
| `.2cba` | Framed blocks of type `0x2000`, one per game | Text, symbols, squares and arrows read |
| `.2lid` | Name lists: players, tournaments, sources, annotators, teams, titles | Players, tournaments, sources and teams read |
| `.2lcd`, `.2lgd` | Catalogue and search structures for the name lists | Not needed for reading |

## 2CBH records

Record 0 is the file header. Its LE16 at byte 10 is the record stride (192).
Its LE64 at byte 16 is the record count, header included (`0x1401ceab0`,
`0x1401cd6a0`). The reader accepts only stride 192.

`0x1401cd6b0` converts a classic 46-byte record into this layout field by
field, which fixes these offsets:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 2 | Flags: classic byte 0 copied unchanged (`0x80` deleted) |
| 2 | 2 | `0x0101` game, `0x0301` text |
| 8 | 8 | 2CBG block offset |
| 16 | 8 | 2CBA block offset (games) |
| 24–56 | 8 each | White, black, tournament, annotator, source ids; all-ones means none |
| 64, 72 | 8 each | White and black team ids (2LID teams); all-ones means none |
| 88 | 1 | Result code (below) |
| 89 | 1 | Evaluation NAG of a line (below) |
| 90, 92 | 2 each | Round, subround |
| 96, 112 | 2 each | White and black rating; Elo is the low 12 bits, as in classic |
| 128 | 2 | Packed ECO (below) |
| 188 | 4 | Date, packed as in classic: `year << 9 | month << 5 | day` |

**Result codes** (both families; classic bytes 27 and 28). The code is
`kind + 4 × forfeit`. Kind 0 is a Black win, 1 a draw, 2 a White win and 3 a
line without a result. The display routine `0x140c41440` reads kind, NAG and a
forfeit flag. It shows forfeits as `-:+`, `=:=` and `+:-`, and a forfeited
line as `0-0`. A line shows the symbol of its evaluation NAG, or "Line" when
there is none. The game-data dialog lists the same seven choices. In the
classic Mega every code 0–7 occurs: 6,543 lines (most with moves), and
2,045 / 129 / 2,763 / 357 games with codes 4–7, nearly all without moves.
Only lines carry a NAG; the values seen are evaluations 10–19 and 36, 40, 44,
132, 138 and 146. The PGN `Result` tag keeps a forfeit's winner and is `*` for
lines and `0-0`.

**Packed ECO** (both families). Bits 7–15 hold 1–500 for A00–E99, and 0 means
no code. The low seven bits are a ChessBase sub-classification that the reader
does not show. This was checked against the opening moves of 24,162 games: all
310 B01 games begin 1.e4 d5, C42 games are Petrov lines and C34 games are
King's Gambits.

## Blocks (2CBG, 2CBA)

Each file starts with a 12-byte header: a LE64 file length and a LE32 word.
Each block then has this layout (`0x1401af990` read, `0x1401afc80` write):

| Relative offset | Width | Field |
|---:|---:|---|
| 0 | 8 | Marker `0x1122334455667788` |
| 8 | 4 | Payload length |
| 12 | 4 | Padding length |
| 16 | 8 | Checksum |
| 24 | 2 | Payload type |
| 26 | n | Payload, then padding, then a LE64 total-length trailer |

The checksum (`0x140f06190`) sums eight chunks of `max(n / 8, 1)` bytes each,
modulo 256. It packs the sums most significant first; any remaining tail bytes
are not covered. The reader rejects a block whose marker, trailer or checksum
does not match. It never returns a partial game.

## Move trees

A payload is a sequence of LE16 words. The rules come from `0x140b4b3f0` and
the table builder `0x140203bb0`.

* `0xfffc` starts a game from the standard position.
* `0xfffb` starts a set-up position. It is followed by:
  * the move number;
  * a word whose low byte is the side to move (1 = Black) and whose high byte
    is the castling rights (1 Q, 2 K, 4 q, 8 k);
  * a word that is zero in every sample (other values are refused);
  * one placement token per piece;
  * `0xfffc`, then the moves.
* **Placement tokens** start at `0xc02d`. They cover white K Q N B R then the
  black pieces, 64 squares each. White and black pawns follow, on the 48
  squares of ranks 2–7. Squares are file-major (`8 * file + rank`). All 597
  set-up games in the sample match their classic setups square for square.
* **Move tokens** index a table rebuilt from the native generator order:
  * pieces by kind and colour, sources file-major, directions in generator
    order;
  * one token per captured-piece alternative;
  * pawn pushes, captures, promotions and a separate en-passant token;
  * the four standard castling moves.

  The reader builds the primary 45,356 entries. Extended sections such as
  Chess960 moves are refused as unsupported.
* `0xfffd` after a move opens a variation on it. Once `0xffff` ends the line,
  the variations follow in reverse order and start from the position before
  their parent move. `0xfffa` is a null move.
* `0xfffe` sets a per-move flag bit. It never occurred in the sample; the
  reader reports it as uninterpreted.

## 2LID name lists

The header is big-endian:
* header size (184) and list count (6);
* per list: slot size, a zero word, record count and two all-ones words;
* an all-ones word, an uninterpreted word (1);
* one `(kind, 1)` pair per list.

The lists are, in order: players (1,024-byte slots), tournaments (1,120),
sources (220), annotators (1,024), teams (314) and titles (532).

Records are stored in fixed groups. Group *n* holds slot *n* of every list, so
a record lives at `184 + n·4234 + (sum of the earlier slots)`. A record is a
LE32 payload length followed by the payload. Strings inside it are
LE32-length-prefixed UTF-8:
* players: last name, then first name;
* tournaments: site, then event. A packed date follows the event.

The other lists:
* sources: title, publisher, then LE32 publication and version dates packed
  as in CBH, then four bytes not interpreted (version and quality in CBS);
* teams: the name, then five bytes. The year is LE16 at +2 and the nation at
  +4, matching CBE; neither is shown;
* titles: a LE32 count of (LE32 language, string) pairs, the titles of text
  entries. Not read yet;
* annotators: empty in the sample. **2CBH annotator ids index the player
  list.** Annotator 17 is the player "ChessBase", for example.

Every player, tournament, source and team reference in the sample matched the
classic names, and so did every annotator. Classic CBC keeps annotators as
"Last,First", and 40 games name an annotator "Mackenzie" where the linked
player is "MacKenzie". The remaining player fields are not interpreted.

## Classic name tables and CBJ

Each classic name table has a 28- or 32-byte header, then fixed records. A
record starts with a nine-byte tree prefix (LE32 left, LE32 right, balance)
and ends with the LE32 game count and first game:
* CBC annotators (62 bytes): a 45-byte name;
* CBS sources (68): 25-byte title, 16-byte publisher, LE32 publication and
  version dates, version and quality bytes;
* CBE teams (72): 45-byte name, LE32 number, season byte, LE32 year, nation
  byte.

Teams are referenced from **CBJ**, which has a 32-byte header (LE32 version 11,
LE32 record size 120, LE32 entry count) and one record per entry from entry 1.
Its first eight bytes are the BE32 white and black team ids; all-ones means
none. The rest of the record is not interpreted.

## Classic set-ups (corrections found here)

* Every classic set-up in Mega Database 2025 has byte 0 = 1. cbh2pgn and libcbh
  both skip this byte, so values 0 and 1 are accepted.
* A stored move number of 0 is read as 1.
* An en-passant file that is impossible on the board is dropped, as ChessBase's
  own 2CBG copy does for entry 13409.

## Annotations (CBA and 2CBA)

Both files hold the same entries. Each entry has a position, a type and data.
The type ids come from the annotation factory `0x1401a2ae0`:

| Type | Content | Shown |
|---|---|---|
| `0x02`, `0x82` | Text after / before the move (`0x20` is a further text kind) | Yes |
| `0x03` | Up to three symbols as PGN NAG numbers: move, position, prefix | Yes |
| `0x04`, `0x05` | Coloured squares (colour, square) and arrows (colour, from, to) | Yes |
| `0x84`, `0x85` | Squares and arrows shown before the move | No |
| `0x07` time, `0x09` training, `0x13` game quotation, `0x14` pawn structure, `0x15` piece path, `0x18` critical position, `0x22` medal, `0x23` variation colour, `0x26` evaluation profile, others | Various | No, counted in a diagnostic |

**Positions.** Position −1 is the start of the game. Position *k* is the
(k+1)-th move, but the two files count moves in different orders:

* **Classic CBA:** stream order, which is the classic CBG order: a move's
  continuation before its alternatives.
* **2CBA:** line order: walk a line, and right after each move number its
  alternative variations completely before continuing.

Two checks support this:
* With the classic order, 94% of "text before" entries land on a variation's
  first move (the base rate is 7%), and 88.5% of arrows start on an occupied
  square. Shifting the alignment by one move either way drops these to
  about 30% and 55%.
* All 16,202 texts that occur once in their game land on the same move from
  CBA and from 2CBA.

**Classic CBA.** A 14-byte header holds the game number (BE24) and the total
length (BE32 at byte 10). Each record has a BE24 signed position, a type byte,
a BE16 length that includes this 6-byte record header, and data
(`0x140190020`, `0x14018fdc0`, `0x14018fd10`, `0x14018f310`). Text data starts
with a flag byte and a language byte.

**2CBA.** The payload repeats a LE32 position and a LE32 count. Each entry is
a LE16 type followed by that class's stream data (vtable slot 4):
* text: flags, language index and a LE32-length string;
* symbols: three bytes;
* squares and arrows: a LE32-length byte string with the classic data.

The payload ends with position `0x7fffffff`. Training (`0x09`), game quotation
(`0x13`) and correspondence (`0x19`) entries nest further structures. The
reader stops at them and reports the game's remaining annotations as not
shown.

**Colours.** Stored colour codes are the PGN importer's colour index plus one
(`0x140e98150`): 2 G, 3 Y, 4 R, 7 B, 8 C, 9 O. Squares are stored 1-based,
file-major. The workbench keeps the `[%csl]`/`[%cal]` letters, and PGN export
writes those commands.

**Text.** Language bytes map to indices through the table read by
`0x140e94770`. Byte 0 means no language; `*`, `5` and `1` (English, German,
French) are confirmed by the texts. Text is UTF-8 when valid UTF-8, otherwise
Windows-1252 (`SString::MayBeUTF8`), and ends at the first NUL. The reader
shows one language per game (English if present, otherwise the most common
language), plus text with no language, and reports the other translations as
omitted.
