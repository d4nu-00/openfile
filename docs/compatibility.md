# Database compatibility

The reader supports classic 46-byte CBH headers and game entries, optional CBP
player and CBT tournament names, and standard classical CBG move streams.
The source database is opened read-only. Text entries are not games.

CBG records with no flags or only `0x40000000` (28-byte setup) are supported.
Unencoded (`0x80000000`), special (`0x04000000`), Chess960 (`0x0a000000`), and
all other flag combinations return an unsupported status without a partial game.
Setup control byte 0, byte 1's upper three bits and byte 2's upper four bits
must be zero; other values return `unsupportedSetup`. The turn bit is `0x10`,
en-passant file is the low four bits (0 absent, 1–8 a–h), and castling rights
use low bits 1/Q, 2/K, 4/q, 8/k. Move number must be 1–255. The occupancy stream
is file-major and MSB-first, with standard chess position validation.

En-passant controls were corroborated against
[libcbh PositionStack::setup](https://github.com/rolandlo/libcbh/blob/main/src/cbh_position.cpp).
This corrects the three-bit mask in the older cbh2pgn reference.

Ordered variations, null moves outside check, filler bytes and modulo-256 move
counters are supported. A terminal root marker is required, with all branch
frames closed. Byte, node and variation-depth limits apply to every import.

Original CBA annotations are not decoded. Comments and NAGs created in the
workbench or imported from PGN use the independent editable game tree.
The newer 2CBH/2CBG format, multimedia and proprietary cloud services are not
supported by this initial reader.

Local validation on 2026-10-11 compared 238 unique selected entries against the
pinned MIT cbh2pgn reference: 235 supported games matched ordered UCI trees,
initial positions, dates and results; three text entries were explicitly
unsupported. Selection combined the 39 retained overlap records (36 games,
three texts) and 200 deterministic spread ordinals. No comparison failures
occurred. Once cloud placeholders were available locally, the largest measured
page and game load took 4.3 ms and 4.2 ms; total native bytes read across the
comparison was 119,946. Peak child RSS was 7.1 MiB. These samples establish
bounded compatibility evidence, not full database or annotation coverage.
Packed ECO codes remain unknown with a diagnostic until their formula is
verified. Cloud-backed files may take substantially longer on first access.
