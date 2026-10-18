# Training annotations

Status: spec written first on branch `training-annotations` (2026-10-18),
stacked on `centipawn-analysis`, before the format change, then
implemented as described. Sections marked *(validated)* were checked against
the test database.

Evidence is from the reference binary (x64), analysed statically through an
REA session. Messages are cited by their ordinal in the message catalogue.

## What the reference app does

| Command | Id | Class | Key | Message |
|---|---|---|---|---|
| Training Annotation | `ANNOTRAINING` (kc 506) | `AnnoTrainingCmd`, run `0x1408e0e80` | Ctrl+Alt+M | "Insert an training question" (50642/50643) |
| Delete Training Commentary | `ANNODELETETRAIN` | `AnnoDeleteTrainCmd`, `0x1409c4920` | | "Delete all training commentary" (4339/4340) |
| Enable Training | | `EnableTrainingCmd`, `0x1409c6230` | | "Ask training questions while replaying the game" (2539/2540) |
| Create interactive training | `CREATETRAINING` (kc 174) | | | 4154/4155 |

* **The annotation** is type 0x09 (`TrainingAnno`, vtable `0x141109500`). It
  holds one *training item*: a `MoveInputItem` (kind 1, vtable `0x141278ff8`)
  or a `MultipleChoiceItem` (kind 2, `0x141279090`), both derived from
  `TrainingItem` (`0x141278f60`).
* **The editor** is dialog template 1301, "Training" (`CTrainingAuthorDia`,
  DoDataExchange `0x1407d0760`, OnInitDialog `0x1407d25e0`). Its controls:
  * the Question text, with language tabs En, De, Fr, Es, It, Ne and Pt
    (`.data` `0x1419264f8`);
  * "Default Wrong", "Help 1" and "Help 2";
  * Seconds, and a Score field with a 0–100 spin;
  * the radio buttons "Move input" / "Multiple choice";
  * Move, New, Replace, Delete and Feedback for the answers;
  * "D <-> E", which swaps German and English.

### The item

The base item (`0x140b94030` reads 2CBA, `0x140b920c0` writes CBA) holds:

| Field | Size | Meaning |
|---|---|---|
| seconds | i32 | the time allowed; 300 in the test database's samples |
| score | i16 | the question's points |
| question | texts | |
| wrong | texts | the reply to a wrong answer ("Default Wrong") |
| help 1, help 2 | texts | two hints |

**Texts** hold 9 slots × 2 strings. Only non-empty strings are stored, each
as (slot, string index 0 or 1, string). *(validated)*
* **Slots 0–6** are the languages of the tabs: English 0 and German 1 as
  the samples show, then French, Spanish, Italian, Dutch and Portuguese in
  tab order.
* **Slot 7** holds arrows to show: (colour, from, to) triples, with the
  colour codes and 1-based squares of arrow annotations.
* **Slot 8** holds coloured squares: (colour, square) pairs.

  These marks go with hints and answers.
* **String index 1** never occurs in the test database. Its meaning was not
  traced.

**Answers** (an u8 count, then each answer):
* move input (`0x140b93d30` / `0x140b91ca0`): from square, to square, an
  extra byte, the answer's points (u8), and its feedback texts.
  *(validated)*
  * The squares are **0-based**, file-major (a1 = 0, a2 = 1, …, h8 = 63),
    unlike the 1-based squares of marks.
  * The question is about the position **before** the move it is attached
    to.
  * All 3,687 move answers of the test database are legal that way; with
    1-based squares 26 are.
  * 3,090 of them are the move played in the game; the others are
    alternatives with their own points and feedback.
  * The extra byte is 0 in 3,667 of them.
* multiple choice (`0x140b93e50` / `0x140b91e40`): the points (u8), the
  choice's texts and its feedback texts.

### The two encodings

**2CBA** (slot 4 `0x140190a10`, then `0x140b93fa0`): a presence byte, a
second presence byte, the kind as LE32, then the item:
* the seconds (LE32) and score (LE16);
* each texts block as an LE16 count, then per string its slot (u8), index
  (u8) and an LE32-length string (at most 0xfff0 bytes);
* the answer count as u8, then the answers as above.

**Classic CBA** (`0x140190190` → `0x140b92060`): the record's data is
* an LE16 presence word (1), the kind (u8);
* the item's length as LE16, which counts itself (`0x140b91ca0` writes it
  last);
* the item as in 2CBA, except that strings have an LE16 length.

The shorts inside the item are little-endian, unlike the record header.

*(validated)* Every one of the classic test database's 3,122 training records
(3,088 move input, 34 multiple choice, in 2,304 games) parses to its exact
length. Entry 22,922's 2CBA copy matches its CBA record field for field.

**Not traced:**
* the extra byte of a move answer;
* string index 1;
* how a question is asked while replaying (Enable Training, "With Clock",
  `CTrainingInput`). The Replay Training spec covers the asking.

## Workbench

### Model

`chess_core::training`:
* `Training { kind, seconds, score, question, wrong, help: [_; 2], answers }`;
* `Block`, a texts block: texts by language (0–6), arrows and squares
  (slots 7 and 8), and any string the workbench cannot represent (index 1, or
  marks with unknown colours), kept as stored;
* `Answer { from, to, extra, points, feedback, choice }`.

It is kept on the node of the move it is attached to, as the other
annotations are, in `Node::training`. Squares are kept as stored and read
as 0-based. Unknown kinds keep the game's annotations marked incomplete, as
before.

### Reading and writing

* **Reading.** Both readers decode type 0x09; a 2CBA game no longer stops
  at a training entry.
* **Writing.** The 2CBA encoder writes it back: both strings of every slot,
  and the extra byte unchanged.
* **Text.** Text bytes go through the same UTF-8/Windows-1252 rule as text
  annotations. Written text is UTF-8.
* **PGN** cannot hold training, so a PGN export asks first, as for critical
  positions.
* **Search.** The search mask's Training filter matches the decoded entry.

### Game window

* **Notation.** A move with training shows a "Training" chip after it. The
  chip opens the question.
* **Training Annotation** (Ctrl+Alt+M) opens the editor for the current
  move:
  * the kind;
  * seconds and score;
  * a language (En, De, Fr, Es, It, Nl, Pt);
  * the question, the wrong-answer text and the two hints in that language;
  * the answers. For move input, a move typed in SAN from the position after
    the move (checked), its points and feedback. For multiple choice, the
    choice's text, its points and feedback.

  Texts in other languages and string index 1 are kept as they were. Saving
  with no question removes the annotation, and so does **Delete**. Each
  change is one undoable edit (`Edit::SetTraining`).
* **Delete Training Commentary** removes every training annotation of the
  game, as one edit.
* Asking the questions while replaying is Replay Training's job, specced on
  its own.

### Measurements

`crates/cbh-reader/examples/training_check.rs`, release build, the classic
test database read-only:
* It loaded the 2,304 games with training: 3,122 questions, 34 of them
  multiple choice, and 4,278 board marks. No game still reports type 0x09
  as not shown.
* All 2,304 games round-trip through the 2CBA encoder and reader with equal
  questions, and nothing is omitted.
* The one game also in the 2CBH sample reads the same from both families.

