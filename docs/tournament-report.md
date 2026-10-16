# Tournament report

Status: implemented on branch `tournament-report` (2026-10-16), stacked on
`player-dossier`. The direction was agreed in conversation: build on the
existing crosstable.

Evidence is from CBase18.exe 18.1 (x64), analysed statically through an REA
session (`analysis/reports/` in the research workspace).

## What ChessBase does

* **The report** is the class `TournamentReport` (vtable `0x1411f7908`),
  another HTML report built on the same generator base as the dossier. Its
  parameters have a dialog, `TournamentReportParamsDia` ("Edit Tournament
  Report Params", vtable `0x1411e0780`).
* **Starting it.** `0x14083b0e0` builds the report for a tournament with
  defaults: all rounds (flag 1), rounds 1 to 9999, one column. It opens the
  result in a report window titled "Dossier: <tournament>" (50087), through
  `0x1407af640`. That is called by `0x14073d970`, slot 0x380 of the
  tournament list's list interface (`CBaseDBGridTournamentListWin`,
  secondary vtable at `0x1411cd018`, this-offset 0x230). Which menu item or
  command calls that slot was not found. No `M_KC_` command names the
  report.
* **The generator** (`0x14083a600`) shows a progress dialog titled
  "Tournament report" (51268). It finds the tournament by name, takes its
  game list (`0x140132cb0`, kind 2) and writes the rounds (`0x14083a800`):
  * For rounds 1 to 19 (or the chosen range, offsets 0x694–0x698) it
    lists the round's games in game-list order.
  * Each round starts with a heading of the round number, the weekday and
    the day of its first game. The heading text is German and hard-coded
    ("Runde n - <weekday>, den <day>.").
  * Each game is a line "White - Black result".
  * An option (0x69c) sets the rounds two per row, in a table.
  * Each round's games are also written as PGN to a hard-coded path
    (`c:\cb\Apps\c10\GamesRound<n>`), with a link to it ("Partien").

  The German text and the fixed path suggest an in-house tool.

**Not traced:** the menu that reaches the report, the parameter dialog's
fields beyond the offsets above, and how ChessBase orders games within a
round.

## Workbench

**Tournament report** (database window Home › Lists, beside Cross Table;
game window Home › Database; and **Report** in the Cross table dialog)
reports on the tournament chosen in the Tournaments list, or the shown
game's tournament. It reads the games as the crosstable does (a sort order
by tournament when there is one, otherwise a search), at most 5,000 games
and 500 players, cancellable through the crosstable's Stop.

The dialog shows:
* **The header:** event, site, players, games, rounds, average rating and
  category.
* **Results:** White wins, draws, Black wins, forfeits, White's score and
  the average game length (the workbench's addition).
* **Standings:** the crosstable's ranking with points, games,
  Sonneborn-Berger and performance.
* **The rounds**, as ChessBase lists them:
  * every round with a number in order, then games without a round;
  * each with its earliest date (unknown parts left out) and its pairings
    "White – Black", with the result and moves, in entry order;
  * ChessBase's options: a range of rounds and **Two columns**. All rounds
    are listed, not only 1–19.
  * **Standings after each round**: the ten leaders after the round, ties in
    the crosstable's order. This is the workbench's addition, on by default.
  * **Games** lists the round's games in the game list, in place of
    ChessBase's PGN file per round; a double-click loads a game.
* **Cross table** opens the tournament's crosstable.

**Not ported:** the HTML page, the German headings and the PGN files.

## Measurements

Release build, `src-tauri/examples/tournament_report.rs`, read-only, with
the app's data folder. On the classic Mega (11.15 M entries), without a sort
order by tournament, three tournaments of 1–35 games took 0.86–0.88 s each,
almost all of it the search for the tournament's games. A sort order by
tournament makes this a lookup.
