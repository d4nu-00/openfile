# Move annotations: medals, move times, diagrams, pawn structures, piece paths, stored evaluations

Status: implemented on branch `annotation-types` (2026-10-22). This spec
was written first. The direction was agreed in conversation.

Evidence is from the reference binary (x64), through an REA session, and from
the test database. Messages are cited by their ordinal in the message
catalogue. See [formats](formats.md) for the CBA/2CBA framing.

## What the reference app does

### Types and layouts

Each class's type id comes from its vtable slot 7; the classic reader is
slot 2, the classic writer slot 1, and the 2CBA reader/writer slots 4 and 3.

| Type | Class (vtable) | Classic data | 2CBA data |
|---|---|---|---|
| `0x06` diagram | `DiagramAnno` (`0x14110a158`) | none (`0x14018d970`) | none |
| `0x07` move time | `TimeAnno` (`0x14110a230`) | BE32 (`0x14018ea50`) | LE32 |
| `0x14` pawn structure | `PawnStructAnno` (`0x14110a470`) | one byte (`0x14018e320`) | one byte |
| `0x15` piece path | `PiecePathAnno` (`0x14110a080`) | bytes to the record's end (`0x14018e490`) | LE32 length and bytes |
| `0x21` stored evaluation | `FritzEvalAnno` (`0x14110a4b8`) | 6 bytes (an older 32-byte form is accepted; `0x14018dbc0`) | 6 bytes |
| `0x22` move medal | `MedalAnno` (`0x14110a500`) | BE32 (`0x14018e200`) | LE32 |

* **Move medal.** Set Medal (`ANNOMEDAL`, kc 509, `AnnoMedalCmd`
  `0x1408e0d60`) opens the Medals dialog that also edits the game's medal
  word, so the bits are the same (see [formats](formats.md#medals): bit 0 Best
  game … bit 15 User).
* **Move time** is a `CBTime` (`MoveTimeVariables`, `0x14008c880`, settings
  key `V_TIME_WHITE`, default `0x01280000`). On real data the four bytes,
  high to low, are hours, minutes, seconds and a fourth byte. In 150,000
  recent test-database games, 8,040 move times have every byte below 60, and
  only 23 a non-zero fourth byte, whose meaning was not traced. Move Time
  (`ANNO_MOVETIME`, kc 168, Ctrl+J, `EditMoveTimeCmd` `0x1409c5d20` →
  `0x1409f0cd0`) edits it: "Edit time used for move".
* **Stored evaluation** (`0x140b557e0`): three LE16 fields in both families:
  the score, the kind (0 centipawns, 1 mate, −1 none) and the depth. These
  are the fields of the evaluation the profile packs (see
  [evaluation profile](evaluation-profile.md)). Tactical Analysis's "Store
  evaluations" writes them (`0x1401a7b10`). Delete Evaluation + Time
  Commentary (`ANNODELETEFRITZ`) deletes them, with move times.
* **Pawn structure** (`ANNOPAWNSTRUCTURE`, kc 514, `AnnoPawnStructureCmd`
  `0x1408e0dc0`, "Visualize the pawn structure for current position") and
  **Piece path** (`ANNOPIECEPATH`, kc 515, `AnnoPiecePathCmd` `0x1408e0e20`,
  "Visualize the path of the current piece up to this position") send window
  commands 0x197 and 0x198 (`0x1408e79f0`). The data:
  * the pawn structure byte is 3 in all 254 found;
  * every piece path found (112) is two bytes: the colour code 3 (yellow, as
    in square marks) and the square the annotated move lands on.
* **Insert Diagram** (`ANNODIAGRAM`, kc 126, `SetOrRemoveDiagramCmd`
  `0x1409c8db0` → `0x1409fba80`) sets or removes the diagram mark.

**Not traced:** how the notation and board draw each type (the window
commands' handlers), the move time's fourth byte, and what other pawn-structure
values would mean.

## Workbench

### Reading and writing

* Both readers decode the six types into the move (or the game's start, for
  position −1). A game with them no longer counts as having unsupported
  annotations.
* The 2CBH writer writes them back in the 2CBA layouts above, unchanged.
  Classic files are never written.
* **PGN.** A move time is written as `[%emt h:mm:ss]` and a stored
  evaluation as `[%eval +0.35,22]` (`[%eval #3,30]` for a mate), and both are
  read back from PGN. Medals, diagrams, pawn structures and piece paths have no
  PGN form, so exporting asks before dropping them.
* **Delete Commentary:** "all" removes them all; "evaluation and time"
  removes move times and stored evaluations.

### Showing

In the notation, after the move:
* medals as their names;
* the move time, e.g. "0:44";
* the stored evaluation, e.g. "+0.35/22";
* a diagram mark;
* "Pawn structure" and "Piece path" marks.

On the board, while such a move is selected:
* **Pawn structure** highlights every pawn's square;
* **Piece path** draws the piece's moves, from where it started up to this
  move, as yellow arrows.

Both are the workbench's own drawing, since the reference app's was not
traced.

### Commands

| Command | Key | Effect |
|---|---|---|
| Set Medal (`ANNOMEDAL`) | | the Medals dialog for the move: the reference app's 16 bits |
| Move Time (`ANNO_MOVETIME`) | Ctrl+J | edit or clear the move's time (h:mm:ss) |
| Insert Diagram (`ANNODIAGRAM`) | | set or remove the diagram mark |
| Pawn structure (`ANNOPAWNSTRUCTURE`) | | set or remove the mark (value 3) |
| Piece Path (`ANNOPIECEPATH`) | | set or remove the mark for the moved piece (yellow, its square) |

Each is one undoable edit.

## Tests

* Reader tests for both layouts of each type, and a 2CBH round trip.
* PGN round trip of move times and evaluations.
* Frontend tests for the commands, the notation marks and the board.
* Validation: the classic and 2CBH test-database families give the same
  entries for their shared range.
