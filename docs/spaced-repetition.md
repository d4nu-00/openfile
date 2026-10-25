# Spaced repetition decks

Status: the workbench's own feature (2026-10-25); the reference app has no
counterpart. Plan: [spaced repetition](superpowers/plans/2026-10-25-spaced-repetition.md).

**Spaced Repetition** (Home › Lists in the database window, Training in the
game window, and the command search) turns a PGN file, or the games of a
database list, into a **deck** of lines. You study the deck day by day: you play
one side's moves on the board, a mistake brings the line back soon, and a line
you know comes back after a growing interval. It is the way Chessable and Anki
train: the board and the lines are yours.

Typical uses: the training databases of [Build Mate Training](mate-training.md)
(or the PGN files made from them), a repertoire PGN with variations, a collection
of puzzle positions (PGN games with a `FEN` tag and the solution moves).

Decks live only in the app's data folder (`srs/decks/<id>.json`). The PGN files
and databases they were made from are only read, never changed. Deleting a deck
asks first.

## Decks and cards

A **card** is one line: a start position (FEN), a list of moves (UCI and SAN) and
the **trainee colour**. You play the trainee's moves; the opponent's moves are
played for you after about 400 ms, so they can be seen. A line whose first move
is the opponent's starts by playing it. Each move keeps the PGN comment after it
(shown once the move is played). The card keeps the source game's White, Black,
Event and Date and a source label: the file name or the database name with the
game or entry number. Annotations other than text comments are not used.

### Lines of a game

With **Include variations** on (the default) every root-to-leaf path of the game
tree is a line; off, only the main line. A path's trailing opponent moves (after
the trainee's last move) are dropped, and a path with no trainee move is not a
card. A line is at most 400 plies; a source with more than 20,000 cards is
truncated and the result says how many lines were left out. A game that cannot
be read is skipped and counted.

### Trainee colour

A deck option. **Side to move** (the default) is the side to move in the game's
start position, which is right for puzzles. **White** or **Black** is for
repertoires; the lines are kept as they are.

### Card id

The id of a card is the SHA-256 (hex, first 16 bytes) of the start FEN's first
four fields, a space, the moves in UCI joined by spaces, a space and the trainee
colour. Equal lines have equal ids, so they are one card (the first is kept), and
the id is the same when the source is read again.

### Accepted moves

At a trainee move the card's move is correct. If you play another move that some
*other* card of the deck plays from the same position (same FEN, first four
fields), the study view says "That move belongs to another line in this deck —
play this line's move" and lets you try again; this is **not** a mistake. Any
other legal move is a **mistake**: "Not this line's move" is shown, the correct
move appears as an arrow, and it has to be played before the line goes on.
A move that is not legal is not accepted at all.

## Scheduling

An Anki-style SM-2 variant. The state of a card is its `kind` (`new`,
`learning`, `review` or `relearning`), the `step` (an index into the learning or
relearning steps), `due` (a time), `interval` (days), `ease` (2.5 to start,
never below 1.3), `reps` and `lapses`.

Deck options and their defaults:

| Option | Default |
|---|---|
| Learning steps | 1, 10 minutes |
| Relearning steps | 10 minutes |
| Graduating interval | 1 day |
| Easy interval | 4 days |
| New cards per day | 20 |
| Reviews per day | 200 |
| Maximum interval | 36500 days |
| New card order | source order (or random) |
| Include variations | on |
| Trainee | side to move |
| Show new lines with arrows first | off |

The steps are comma-separated minutes in the options dialog, which checks them
(learning steps cannot be empty; relearning steps may be).

### Grades

**Again**, **Hard**, **Good** and **Easy**. The grade is partly decided by how
the line went:

* A card played with a **mistake** is graded **Again**, after you have completed
  the line, whatever the grade sent. The study view shows "Next card".
* A card completed with a **hint** and no mistake is graded **Hard** at most:
  Good and Easy are disabled.
* A clean card shows Again, Hard, Good and Easy with the next interval under
  each, and Good is the default (Enter). The keys 1 to 4 choose the grades.

The backend enforces both rules, so a client cannot record a mistake as Good.
The intervals under the buttons come from the same code as the answer.

### New, learning and relearning cards

With steps `S` and the current `step`:

* **Again**: `step = 0`, due after `S[0]`.
* **Hard**: due after `S[step]`, the same step; for the last or only step, after
  `1.5 × S[step]`.
* **Good**: `step + 1`, due after `S[step + 1]`, or **graduate** when that was
  the last step.
* **Easy**: graduate as Easy.

Graduating from `new` or `learning`: the card becomes a `review`, with the
graduating interval (Good) or the easy interval (Easy). Graduating from
`relearning`: the card becomes a `review` and its interval stays as the lapse
set it. A `new` card that is answered becomes `learning` at step 0 first, and
then the rules above apply. Graduating does not count a repetition or fuzz the
interval.

### Review cards

* **Again**: `lapses + 1`, `ease - 0.20`, `interval = max(1, interval × 0.5)`, the
  card becomes `relearning` at step 0 and is due after the first relearning
  step. Without relearning steps it stays a review, due after its new interval.
* **Hard**: `interval = max(interval + 1, interval × 1.2)`, `ease - 0.15`.
* **Good**: `interval = max(interval + 1, interval × ease)`.
* **Easy**: `interval = max(interval + 1, interval × ease × 1.3)`, `ease + 0.15`
  (the interval uses the ease from before).

Then the interval is limited to the maximum interval and **fuzzed**: an interval
of 3 days or more varies by up to ±5% with a number taken from the card id and
its repetitions, so equal cards spread out and the same answer gives the same
result. `due = now + interval days` and `reps + 1`. A lapse also counts a
repetition. The ease is never below 1.3.

### The day

A day starts at **04:00 local time**. "Due today" means due before the next
04:00. The frontend sends the time and the local offset from UTC with every
command that schedules, so the backend never reads the clock.

## The study queue

At a time `now` the queue is:

1. learning and relearning cards with `due ≤ now`, by due time;
2. review cards due today, by due time, up to the reviews left today;
3. new cards, up to the new cards left today, in the deck's new card order.

Learning cards come first. Reviews and new cards are interleaved: one new card
after every `max(1, reviews / new)` reviews. The reviews and new cards answered
today (since 04:00) are counted from the deck's log for the daily limits;
learning steps use neither limit. After each answer the queue is worked out
again, so a card that becomes due during the session comes back.

When nothing is due but a learning card is due within 20 minutes, it is shown
early ("learning ahead"). Otherwise the session ends. Suspended cards are never
in the queue.

The **random** new card order is stable: it is seeded from the deck id.

### New cards

By default a new card (the **New** badge) is tried like any other: nothing is
shown before you play, a mistake reveals the move and makes the card Again, and
a clean line offers Hard, Good and Easy.

With the deck option **Show new lines with arrows first**, the first time a new
card is shown it is a lesson instead (the **Learn** badge): each trainee move is
first shown as an arrow and you play it. A wrong move only shows the arrow again;
no mistake is counted. The card is then answered as Good automatically, which
moves it to the second learning step: it comes back after 10 minutes with the
default steps, and then graduates. This suits a repertoire you have not seen
before; for puzzles, leave it off.

## Study view

A modal named "Study <deck name>". The board is turned for the trainee. The
heading says whose move it is and, when the line ends in mate by the trainee,
how many of their moves it takes: "White to move · Mate in 3". It shows
the card's source label and players, the counts at the top (New, Learning, Due)
and, after a line, the grade buttons. Moves can be clicked or dragged;
promotions ask for the piece. **Hint** shows the arrow of the move (and caps the
grade at Hard), **Suspend card** takes the card out of the queue, and **End
session** (or Escape) shows a summary: cards studied, how many were Again, and
the time.

## Stats

For each deck: the number of cards; new, learning and suspended cards; review
cards **due today**; **mature** cards (interval of 21 days or more); the reviews
answered today; the **retention** over the last 30 days (the share of review-card
answers that were not Again); and a 7-day forecast of review cards due (today
includes the overdue). The decks table shows Cards, New, Learning, Due and
Retention.

## Making and updating decks

* **New deck from PGN…** reads a PGN file. The games are read one by one, so one
  game that does not parse does not stop the others; the number skipped is
  shown.
* **New deck from list** (when a database is open) reads the games of the list
  with a separate handle, so the list stays usable: **All games in the list** or
  **Selected games**.
* **Update from source** reads the source again. The cards whose ids still exist
  keep their progress (and a card you suspended stays suspended); new lines are
  added; lines that are gone are **suspended**, not deleted, with their progress
  kept, and they come back when a later update has them again. The result says
  how many were added, kept, suspended and restored. A PGN deck is updated from
  its file; a list deck from the games of the list that is open. An update
  that finds no lines to study changes nothing.
* **Options** edits the name and the options above. Include variations and the
  trainee colour apply the next time the deck is updated; the new card order
  applies at once.
* **Delete** asks first and removes the deck's file.

## Storage

Each deck is one JSON file in `srs/decks/` of the app data folder, written to a
temporary file and then moved into place. A file that does not parse, or has a
newer `version` than this build knows, is listed as unreadable with its file
name and is **never written**; a command that would change it fails. Such a file
can still be deleted from the file system. Large decks (thousands of cards) are
rewritten as a whole after each answer.

## Mate chains

The databases made by [Build Mate Training](mate-training.md) hold each mate as a
mate in 1, then in 2, and so on, game by game. Make a deck from the **chains**
(`<name> - Mate chains`, as a database list or as a PGN) and keep the new card
order at **source order**, so that a mate in 1 is learnt before the mate in 2 of
the same game. The side to move is the mating side in every puzzle, so the
default trainee fits.
