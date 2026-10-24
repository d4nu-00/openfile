# Mate training builder

Status: the workbench's own feature (2026-10-24); the reference app has no
counterpart. Plan: [mate training builder](superpowers/plans/2026-10-24-mate-training-builder.md).

**Build Mate Training** (database window, Home › Lists) turns the games in the
list into training databases of forced mates in 1 to 5 moves. The list is
usually a search for games that end in mate. Every puzzle is verified as a
forced mate in exactly N moves with a single mating first move, and is stored
with training questions, so **Replay Selected Games** ([Replay Training
Sessions](replay-training.md)) quizzes the user and keeps progress.

## What it builds

For `<name>` and the folder chosen:

| File | Contents |
|---|---|
| `<name> - Mate in 1.2cbh` … `<name> - Mate in 5.2cbh` | one database per depth in the range chosen that has a puzzle |
| `<name> - Mate chains.2cbh` | for each source game, the same mate as a mate in 1, then in 2, … up to how far it stayed a verified mate (backward chaining) |

The databases are new files. A build is refused when any of them exists, and
nothing is ever written next to a source or over an existing database. The
files are written under temporary names and moved into place when all are
complete; a failure leaves none of them.

## Definitions

* **Chain.** Let the source game's main line end in checkmate after ply `L` (the
  last move is the mating side's). The **step-i position** is the position
  before ply `L - 2(i - 1)`, that is `2i - 1` plies before the end, for
  `i = 1..5`, when it exists. The mating side is to move there; step 1 is the
  position before the mating move. Chess960 games and games whose main line does
  not end in mate are skipped.
* **Valid step.** Step `i` is valid when, in the step-i position, (1) the side
  to move has a forced mate in exactly `i` moves, none shorter, (2) exactly one
  first move forces mate in `i` (the puzzle is **unique**), and (3) that move
  is the game's move. The chain of a game is steps `1..k`, where every step up
  to `k` is valid and step `k + 1` is not, does not exist, or is beyond the
  deepest depth asked. A chain stops at its first invalid step; later steps are
  never used. Because every step is valid, every mating-side move of a step-i
  puzzle is the unique fastest mate at its point, so each is a training
  question.
* **Combination.** The pieces involved in the final mate
  (`cbh_reader::ending::mating_pieces`, see [search](search.md#ending-tab-2026-10-22))
  written in the order Q, R, B, N, P, K joined by `+`: `Q+N`, `R+K`, `Q+B+P`,
  `N`. The king is part of the combination when it takes part in the mate.
  Bishops and knights are distinct unless **Bishop = Knight** is on, which
  writes both as `m` (`Q+m`). A list of combinations is compared after
  normalising order, case and spaces (`n + q` is `Q+N`).
* **Quota.** `Puzzles per combination` puzzles for each (combination, depth) in
  the depth databases, and that many chains (of two or more steps) for each
  combination in the chain database. The scan ends when the games run out, or,
  with a list of combinations, when every quota is met. A game is not verified
  when nothing it could add has room; a full quota at one depth does not stop
  a chain from being verified further while a deeper depth, or the chains,
  still have room.
* **Duplicates.** A puzzle is identified by its position (board, side to move,
  castling, en passant). A position already collected for a depth is skipped, as
  is a chain already collected (the same top-step position and moves). Large
  databases hold many duplicate games.
* **Order.** In each depth database the combinations are interleaved in
  order of first appearance: the first puzzle of each combination, then the
  second of each, and so on, so practice mixes patterns. The chain database
  keeps each chain together, mate in 1 first, chains in the order found.

## The puzzle games

A step-i puzzle starts at the step-i position (`SetUp`/`FEN`) and plays the
source game's remaining `2i - 1` plies, with no variations. The tags White,
Black, Event, Site, Date, Round, Result, WhiteElo and BlackElo are kept; Annotator
is `OpenFile mate training`. A comment before the first move reads
`Mate in 3 · Q+N · from game 12345` (the source entry number).

There is a move-input training question on each mating-side move (`i` of
them), with the move as its only answer (1 point, question score 1, no time
limit). The first question reads `White to move: mate in 3`, later ones
`Continue: mate in 2` and `Mate in 1`; the first carries the hint `Pieces in
the mate: Q+N`; a wrong answer says `Not the mate. Look at the squares around
the king.`

A training answer stores only the move's squares (see [training](training.md)),
which the replay reads as a queen when a pawn promotes. A mate by promotion to
a queen round-trips; a puzzle in which a mating-side move is an
**underpromotion** cannot be asked, so the game stops its chain there and the
step is counted as "mate by underpromotion". Likewise a step position the
databases cannot store as a start position (one in which an en-passant capture
is legal) ends the chain before it and is counted as "unstorable".

## How steps are verified

* **Mates in 1 and 2** are solved exactly (`chess_core::mate`), with no engine:
  all legal moves, and for a mate in 2 every reply of the opponent. A move
  that stalemates is not a mate. Draw rules (repetition, the fifty-move rule)
  are not considered.
* **Mates in 3 to 5** are checked with a UCI engine, such as Stockfish, with
  `MultiPV` 2 and the engine's own defaults for Threads and Hash (the options
  saved with the engine in Engine Management, and tablebases, are not used,
  because a tablebase win is not scored as a mate). Each position is
  searched until the best line is `mate n` at a depth of at least `2n + 4`
  with the second line searched as deep, or until **Time per check** ends.
  The step is valid when the best line is a mate in exactly `n`, the second
  line is not a mate in `n` or fewer, and the best line's first move is the
  game's. A position with a single legal move has no second line, which is
  unique. A search that ends with no score is a timeout, never valid.

A uniqueness check with an engine can only see what the engine finds in its
time: a second mating move that the engine has not reached is missed, so a
puzzle can be accepted that has a second, longer-to-find mate in `n`. Raising
the time makes the check stricter. Mates in 1 and 2 do not have this limit.

A game that is not used, and why, is counted by kind: not a forced mate, a
shorter or a longer mate, more than one mating move, a mate with another
move, no engine score in time, a game that does not end in mate, Chess960,
an unreadable game, a mate by underpromotion, a position the databases cannot store (a legal en-passant
capture), and a position already collected.

## Workflow

1. In the database window search for the games, for example **Game data ›
   Final position › Mate**, perhaps with Year and Elo, or the **Ending** tab's
   **Mate by**.
2. **Build Mate Training**: choose all games in the list or the selected
   rows, the depths, puzzles per combination, optionally the combinations,
   the engine and its time per check, a folder and a name. **Stop** keeps what
   has been found and writes it; **Cancel** writes nothing.
3. Open a `Mate in N` database from the summary, **Select All**, **Replay
   Selected Games**.
4. For backward chaining, replay the chains database in order: each chain goes
   from the mate in 1 to its longest step.

## Command line

`cargo run --release --example build_mate_training -- --path <db> --filter
'<GameFilter JSON>' --engine <exe> --millis 800 --min 1 --max 5 --per 20
[--combinations "Q+N,R+K"] [--pool-minors] [--minutes 40] --out <folder> --name <name>`
(`--minutes` stops the build after that long and keeps what was found, as
Stop does) runs the search, the same builder and writer, prints progress lines and a table of
the counts by combination and depth.

The real-engine tests run with `OPENFILE_TEST_STOCKFISH=/path/to/stockfish
cargo test -p openfile --test mate_training -- --ignored`.
