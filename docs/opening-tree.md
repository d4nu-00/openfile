# Opening tree and reference search

Status: implemented on branch `opening-tree` (2026-10-13). Direction agreed in
conversation: build the tree from the user's own databases, store it in the
app's data folder like the search booster, and answer the reference search
from it.

## What ChessBase shows

ChessBase's book pane (game window, `OPENBOOK` Ctrl+F12, `TOGGLEBOOKPANE`
Ctrl+Shift+F12 for the extra pane outside the notation tabs) lists the moves
of the current position. The column headers are plain strings in
CBase18.exe 18.1, added in this order by the code at `0x1409aef30`:

| Header | String VA | Shown |
|---|---|---|
| N | `0x14110e2b4` | always |
| % | `0x1410ffc40` | always |
| =% | `0x141244164` | always |
| Av | `0x141244168` | when a global display flag is set |
| Perf | `0x14124416c` | with Av |
| Fact | `0x141244174` | book weights only |
| Prob | `0x14124417c` | book weights only |
| [%] | `0x14112e200` | book weights only |

Width samples beside them (`0x1412440e8`–`0x141244150`, used at
`0x1409ac25e`–`0x1409ac81e`) show the formats: `33333333` games, `100.0%`,
`2800` for Av, `+123*` for Perf. The settings keys `ShowPerformance` and
`PerfAbsolute` (`.rdata` file offsets `0x1115700`, `0x1115710`) switch Perf
between an absolute rating and a difference. `M_TREE_WHITE_SIDE` "Result from
white side" (message 3210) is the option to score from White's side rather
than the side to move. `M_LAST_PLAYED` "Last played" (50001) is the year
column. Fact, Prob and [%] describe an engine book's learned weights, which a
tree built from games does not have, so the workbench leaves them out.

The FIDE rating-difference table (0, 7, 14, … 677, 800 for 50 %–100 %) is in
`.data` at `0x141978ad0`. The workbench computes Perf with that table; its use
in the book pane itself was not traced.

The reference search (`TOGGLEREFERENCEPANE` Ctrl+Shift+F7, "Show/Hide the
automatical search pane"; `FINDREFDBBOARD` Shift+F7, "Search the current board
position in the reference database") lists the games of the reference
database that reach the board, and follows the game as you move.

## Columns in the workbench

For each move played in the current position:

| Column | Meaning |
|---|---|
| Move | The move; click to play it |
| N | Games that played it, and their share of the position's games |
| % | Score of the side to move, over games with a known result |
| =% | Draws among games with a known result |
| Av | Average rating of the player making the move, over rated games |
| Perf | That player's performance: the opponents' average rating plus FIDE's difference for the score, over games with a rated opponent and a known result |
| Last played | The latest year; the first year is in the tooltip |

Games that end in the position are counted in the total and listed by the
reference search, but have no move row. Forfeits count as their PGN result, as
in the search mask; "0-0" and unknown results count as games but not in %.

## Building the tree

Create Opening Tree (Maintenance) builds the tree of the open database into
the app's data folder (`opening-trees/`, one `.cwtree` file per database),
never beside the database. Only main lines are read. Variations are analysis,
and their games did not reach the position over the board.

* **Positions.** A position is the board and side to move, hashed to 64 bits
  with the workbench's own Zobrist keys. Castling rights, en passant and move
  numbers are ignored, as in Find Position. Positions after 0 to 60 half-moves
  are considered. A position repeated within one game counts once.
* **Which positions are stored.** Only positions that the main lines of at
  least 5 games reach are stored. This keeps the file small: in a 446,000-game
  sample of the classic Mega, requiring 5 games instead of 2 cut the stored
  positions from 590,829 to 153,847 but the stored game references only from
  7.9 M to 6.8 M.
* **What is stored.** For each stored position and each next move: games,
  wins, draws and losses for the side to move, the rating sums behind Av and
  Perf, and the first and last year. Then the games themselves, as
  delta-coded entry numbers. A move played in a single game is stored as just
  that entry number; its statistics come from the game's header when read.
* **How it is built.** First, every main line is decoded once, in parallel
  (one reader per thread), into a temporary file of packed moves. Then the
  positions are replayed from that file on a small board that keeps the hash
  up to date. They are sorted one hash range at a time, at most 512 MB at
  once, so memory stays bounded for any database size. The tree is written in
  hash order with a directory and a sparse summary, so a lookup takes two
  small reads. A cancelled build leaves nothing behind.
* **Staleness.** The tree records the database's entry count and the size and
  time stamps of its entry index and move file. It is not used once those
  change outside the workbench. Its own saves are recorded in a journal (see
  below).

## Answering a position

The pane sends the current line: the game's start position and the moves
leading to the current node.

1. If the tree stores the position, its rows and games are read directly.
2. Otherwise the line is walked back to the last stored position. The games
   that played the line's next move there are replayed, at most 5,000, and
   those whose main line reaches the current position are counted. Since an
   unstored position has fewer than 5 games, this is a handful of games
   within the first 60 half-moves. Beyond that limit it can be more. This is
   also how the pane follows a single game deep into the middlegame.
3. If no position of the line is stored, for example from a set-up start, or
   too many games would need replaying, the reference search falls back to
   Find Position (with the search booster when there is one). Without a
   booster, a database of more than 200,000 entries is not scanned until
   the user asks (**Search all games**): on the classic Mega such a scan takes
   23 minutes (see [game window](game-window.md)). The book pane says the
   position is not in the tree.

**Known gap.** A game that reaches an unstored position by another move order
is not found in step 2. Checked against a full scan of every main line, 2,965
of 3,000 random lines from the 2CBH Mega were exact. The rest missed such
transposed games or started from set-up positions, and no answer listed a
wrong game. Find Position (Hard Disk) always searches every game, variations
included.

## Reference search

The Reference pane lists the games that reach the current board, in entry
order, at most 100,000. It is answered from the tree as above, or by Find
Position when there is no tree. Every move starts a new request; the previous
one is cancelled, so a slow fallback search never holds up the board. Opening
a game from the list selects the position found.

The reference database is the open database. ChessBase's separate reference
database setting is not implemented yet.

## Saving into a database with a tree

When the workbench saves, replaces or deletes a game in a 2CBH database it
writes, it records the change in a journal beside the tree (`.cwtj`). A replaced game's
old main line is kept. Answers then drop the changed games' old lines and add
their new ones, so the tree stays current without a rebuild. The journal
records the database's stamps after each change. A paste records every
game it appended, as long as the journal stays within its 10,000 entries;
a larger paste makes the tree stale. Any change made by another program
makes the tree stale as before.

## Measurements

Release build on the development Mac (10 cores, 16 GB, warm cache; 8 decoding
threads). Details are in [validation](validation.md).

| Database | Games | Build | Tree file | Stored positions |
|---|---|---|---|---|
| Mega 2025, 2CBH family | 24,759 | 3.3 s | 0.8 MB | 8,085 |
| Mega 2025, classic | 11,148,461 | 9 min 45 s | 686 MB | 3,576,062 |

The classic build peaked at 0.8 GB of memory. Its temporary file of packed
main lines (about 1.3 GB, estimated from its record size of 6 bytes plus 2 per move) is deleted at the end. A stored position is answered
in under a millisecond. Listing the first 100,000 games of the start position
takes about 25 ms.
