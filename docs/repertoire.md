# Generate Repertoire and the Repertoire Report

The personal [Repertoire workspace](repertoire-workspace.md) is separate from
these generated 2CBH databases and reports. It stores manually selected White
and Black moves in the app data folder, without modifying a source database.

Status: implemented on branch `generate-repertoire` (2026-10-18). The direction
was agreed in conversation: trace what the reference app writes, then build
it on the player's main lines and write new 2CBH databases.

Evidence is from the reference binary (x64), analysed statically through an
REA session. Messages are cited by their ordinal in the message catalogue.

## What the reference app does

The reference app has two commands with this name, both in the database
window's Report tab:

| Command | Id | Class, run slot | Message |
|---|---|---|---|
| Generate Repertoire | (kc unnamed) | `GenerateRepertoireDBsCmd`, `0x140899500` | "Generates Repertoire databases for White/Black from your own games." (51241/51242) |
| Repertoire Report White | `GENERATEREPERTOIRE` (kc 720) | `GenerateRepertoireCmd`, `0x140899450` | "Scan the database for games of your repertoire and generate a report" (50454/50455, 51233) |
| Repertoire Report Black | | `GenerateRepertoireBlackCmd`, `0x1408993a0` | 51234 |

### Generate Repertoire (`0x14070e970`)

* If a single repertoire database already exists, it offers to split it
  into White and Black (51239, `0x14070dad0`, not traced).
* A choice dialog ("CreateRepChoice", `0x14070bfa0`, title 51118) offers
  three sources: the user's games on the vendor services (51116), a
  database with the user's games (51117) or an empty database (51152).
* For White and then Black it creates "Repertoire White" and "Repertoire
  Black" (`0x14079b440`) as classic `.cbh` databases in its repertoire
  folder, and registers them as the repertoire databases.
* **Filling one** (`0x14070ce90`):
  1. It counts the players of the source database over its games and keeps
     the 8 most frequent (`0x140b19720`). With more than one, a dialog asks
     which are the user ("Select player names", 51115, `0x140710ad0`).
  2. It loads each chosen player's games of the colour and puts each into
     one of 500 buckets by the ECO code of the reference app's own
     classification (`0x14070caf0`, `0x140b549c0`). Games under 3 plies go to
     bucket 0, which is never written.
  3. For each code with at least 2 games (`0x140712500`, progress "Perform
     clustering", 51119) it writes one game (`0x140711a20`). From 16 games on,
     it first clusters them with its opening clustering (`0x140711c30`, with
     parameters 3 and 2000, not traced) and writes one game per cluster.
  4. **The merge** (`0x140711f20`):
     * The first game is the base, and each other game is merged into it to
       24 plies (`0x140b64240`).
     * Every annotation except critical positions is removed first
       (`0x140b65c80`, mask `0xfeffffff`), and critical opening marks are
       removed from the opponent's moves.
     * Each game is marked (`0x14070fcd0`). The move at the last ply its
       classification matched, moved to the player's side by parity
       (White `& ~1`, Black `| 1`), gets a critical opening position
       (annotation 0x18 kind 1). This is the "blue" move of the reference
       app's how-to (51120–51122).
     * The game's ECO is the code, with the cluster in the low bits.
     * The White and Black name fields hold the ECO table's opening name,
       split at a move number and without parentheses (`0x14070f020`).
* If neither database could be made: "Can't create repertoire database"
  (50456).

### Repertoire Report (`0x14070f570`)

* It reads the repertoire database of the colour, at most 5,000 games
  (warning 50912, `0x14070c340`). In each game it collects every move with
  the critical opening mark, variations included (`0x14070cc60`). A game's
  topic is its first text, or "White (Black)" from its names
  (`0x14070f830`).
* It scans the database (`0x14070d5e0`, progress 50085 "Repertoire Scan").
  For each game that is not deleted, it takes the first marked position, in
  repertoire order, that the game contains. Each position keeps its best
  100 games, by a score (`0x140464ca0`):
  * the rating curve 89.42282 / (((2850 − r) / 308.9042)^4.364275 + 1) +
    6.76487, the one Prepare uses. Here *r* weighs the side to move twice:
    (2·mover + other) / 3, with 2700 for unrated players found in a list;
  * multiplied by an age factor (`0x140464bc0`: months between the game's
    date and a reference date, then (months / 101.7997)^0.6718763);
  * plus 200 when header flag bit 1 is set.
* The HTML page (`0x14070e2d0`, `0x140710050`) has the title, the database
  name, the number of games, "Repertoire topics: n.", the how-to lines, and
  per topic its positions with a diagram and their games ("All games (n)").

**Not traced:** splitting an existing repertoire database, the clustering
and its parameters, the reference date of the age factor, the list of
players who count as 2700, and the meaning of header flag bit 1.

## Workbench

### Critical positions

* **Format.** The game model now keeps the critical position annotation
  (0x18): one byte, 1 opening, 2 middlegame, 3 endgame. It is read from CBA
  and 2CBA and written to 2CBA.
* **Display.** The notation shows marked moves on a tinted background with
  a tooltip; opening marks are the reference app's "blue" moves.
* **PGN.** PGN has no field for them, so a PGN export of a game with marks
  asks before leaving them out, as for undecoded annotations.
* **Editing** them (the reference app's `ANNOCRITICALPOS*` commands) is not
  part of this change.

### Generate Repertoire

* Generate Repertoire (database window, Report › Report) builds from the open
  database, which is the reference app's "Select database with my games".
  The vendor-services source is left out, and so is an empty database (New
  database already makes one).
* It lists the database's 8 most frequent players from the Players list
  (built first if needed), with the most frequent ticked. The user ticks
  their own names, up to 8, and chooses a folder.
* The job runs in the backend, cancellable with progress, and writes nothing
  when stopped:
  1. It finds the chosen players' games, deleted ones left out, and splits
     them by colour.
  2. It reads the main lines of the latest 20,000 per colour, 60 plies each,
     from the standard start position only. Set-ups count as not
     classified.
  3. It classifies each line with the open ECO table (see
     [openings](openings.md)), and groups the lines by code.
  4. Codes played once are left out (counted as rare).
  5. From 16 games on, a code is split by the table entry each game
     reaches. Entries played once join the code's most played entry. This
     stands in for the reference app's clustering.
  6. Each group is merged into one game to 24 plies, in entry order, with
     each line's repertoire move marked by the reference app's rule.
     Repertoire games are capped at 60,000 nodes.
  7. Names come from the table entry of the merged game: the family before
     ":" goes in White, the variation after it in Black. The ECO tag is the
     code; the sub-code is not written. The event is "Repertoire White" or
     "Repertoire Black".
* It writes "Repertoire White.2cbh" and "Repertoire Black.2cbh", never
  classic files, into the chosen folder. It refuses when either already
  exists. A side without a repertoire game writes no file.
* The result shows, per side: the games, the repertoire games written, the
  games merged into them, the rare and the not classified. **Open** opens a
  database.
* Unlike the reference app, every game is cut at 24 plies, the base game
  too. A repertoire database then holds openings, not whole games.

### Repertoire Report White and Black

* Report › Report, small buttons. The dialog asks for the repertoire database.
  It starts from the one generated in this session, if any. The reference
  app's own repertoire database setting is not ported.
* It reads at most 5,000 repertoire games, as the reference app does, and
  at most 20,000 marked positions.
* It scans the open database's latest 2,000,000 entries, main lines to 40
  plies, cancellable with progress. Deleted, unreadable and set-up games are
  left out.
* **Each game goes to the deepest marked position it reaches**, with ties
  in repertoire order. This is the workbench's choice. The reference app
  takes the first in repertoire order; its marks go as shallow as 1.e4 (for
  codes whose table line ends there), so on the test database that put almost
  every game under the first few topics: 32 of 400 topics had games, against
  400 with the deepest position.
* Each position keeps its best 100 games by the reference app's rating
  curve. The age factor and the +200 for flag bit 1 are left out, since they
  were not traced.
* The dialog shows the topics in repertoire order: each position's moves, its
  number of games and its best 10 by name. Double-clicking a position lists
  its best 100 in the game list; double-clicking a game loads it. At most
  1,000 positions are shown.

### Architecture

* `crates/cbh-reader/src/repertoire.rs`:
  * `build` (pure, from UCI lines, with unit tests) and
    `marked_positions`;
  * `Database::uci_lines` and `Database::first_marked`, both on the shared
    parallel reader of Prepare.
* `src-tauri/src/repertoire.rs`:
  * `frequent_players`, `generate_repertoire`, `repertoire_report` and
    `cancel_repertoire`, keyed by database;
  * listing a position's games reuses `show_entries`.
* Frontend: `GenerateRepertoireDialog` and `RepertoireReportDialog` in
  `src/components/RepertoireDialogs.tsx`, with mocks in the harness and the
  demo API.

## Measurements

Release build, `src-tauri/examples/generate_repertoire.rs`, the classic test
database (11,151,119 entries) read-only, warm, player A (6,925 games):

| Step | Time | Result |
|---|---|---|
| Player list (cached) and the 8 frequent players | 0.02 s | |
| Generate both databases | 1.14 s | White: 3,515 games, 468 repertoire games from 3,405, 65 rare, 45 not classified. Black: 3,410 games, 462 from 3,290, 77 rare, 43 not classified |
| Report White over the latest 2,000,000 games | 25–38 s | 1,252 positions; 1,987,126 games reach one; 400 topics with games |
| Report Black | 24.7 s | 1,483 positions; 1,774,489 games; 421 topics |

The databases take about 2.3 MB each on disk, most of it the name list. Six
White and nine Black lines stopped early at a move the workbench could not
add to the tree. The first run, which took the first position in repertoire
order as the reference app does, showed 32 White topics with games.
