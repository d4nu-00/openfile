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
