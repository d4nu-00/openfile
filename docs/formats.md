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
| `.2cba` | Framed blocks of type `0x2000`, one per game | Not decoded yet |
| `.2lid` | Name lists: players, tournaments, sources, annotators, teams, titles | Players and tournaments read |
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
| 88 | 1 | Result: 0 `0-1`, 1 `½-½`, 2 `1-0`; other codes not interpreted |
| 89 | 1 | Classic evaluation byte (not interpreted) |
| 90, 92 | 2 each | Round, subround |
| 96, 112 | 2 each | White and black rating; Elo is the low 12 bits, as in classic |
| 128 | 2 | Packed ECO (below) |
| 188 | 4 | Date, packed as in classic: `year << 9 | month << 5 | day` |

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

Every player and tournament reference in the sample matched the classic CBP
and CBT names. Other record fields, and the source, annotator, team and title
lists, are not read yet.

## Classic set-ups (corrections found here)

* Every classic set-up in Mega Database 2025 has byte 0 = 1. cbh2pgn and libcbh
  both skip this byte, so values 0 and 1 are accepted.
* A stored move number of 0 is read as 1.
* An en-passant file that is impossible on the board is dropped, as ChessBase's
  own 2CBG copy does for entry 13409.
