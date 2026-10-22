# Feature checklist

This file lists the feature surface of the reference app, an established desktop
chess database, so OpenFile can cover it over time. It is built from the command table (806
`M_KC_*` commands), the ribbon builders, the shipped message catalogues, dialog templates and earlier format
research. See [docs/ui-shell.md](docs/ui-shell.md) for how these were recovered.
Names in backticks are the reference app's command ids, for traceability.

**Status:** `[x]` done in the workbench · `[ ]` not started · *(partial)* noted inline.

**Priority:** **P1** core database and analysis work · **P2** strong value · **P3** niche.

**Portability:**
* 🟢 Can be built locally from the user's own files.
* 🟡 Needs open data or an open service in place of the reference app's.
* 🔴 Depends on a vendor service or account: replace it or leave it out.

## Suggested order

1. Read the newer 2CBH format, decode CBA annotations and write databases (§1).
2. Search mask and position search with a local index (§4). *(P1 items done)*
3. Opening tree built from the user's databases, and the reference search (§8).
4. Complete notation editing: variation operations, game data, symbols (§6). *(P1 items done)*
5. Multiple game tabs, the clipboard database and save/replace into a database (§2, §3). *(game tabs done)*
6. Engine tools: blunder check, deep analysis, analysis jobs, tablebases (§7). *(kibitzers, lines, paste, Engine Management, Tactical Analysis, tablebase tab and annotation, Deep Analysis and analysis jobs done)*
7. Reports, preparation and training (§9–§11), then publishing (§12). *(player statistics, preparation, dossier, tournament and error reports done)*

## 1. Database formats and storage

- [x] P1 🟢 Read classic CBH header records, players (CBP) and tournaments (CBT)
- [x] P1 🟢 Decode classic CBG move streams with variations and setup positions
- [x] P1 🟢 Decode CBG Chess960, special and unencoded game records *(the encoding kinds were traced in the reference binary (`0x1401fb8f0`, `0x1401fc8b0`). All 892 Chess960 games with a set-up and all 11 special records of the classic test database decode. Chess960 castling is in chess-core; PGN carries `Variant "Chess960"`; engines get `UCI_Chess960`. Saving Chess960 to 2CBH is refused (2CBG encoding not traced). The Chess960 record without a start position, and two-byte records with variations, are refused. The test database has no unencoded records. See [formats](docs/formats.md#classic-cbg-encoding-kinds-and-chess960))*
- [x] P1 🟢 Read the newer 2CBH/2CBG format *(records, blocks, standard and set-up move trees, 2LID players and tournaments; all 24,759 test database games cross-checked against the classic file)*
- [x] P1 🟢 Decode CBA annotations: text, symbols, arrows, coloured squares and training *(classic CBA and 2CBA; training, quotations and other kinds are reported, not shown)*
- [x] P1 🟢 Decode annotators (CBC), sources (CBS), teams (CBE) and other companions *(and the 2LID source and team lists; result codes 3–7; titles and remaining CBJ fields not read)*
- [x] P1 🟢 Write databases: save new games, replace games, mark deleted (`SAVEGAME`, `REPLACEGAME`, `DBLISTSAVEGAMECMD`) *(2CBH only; the reference app's .2lcd/.2lgd indexes are left to its integrity check; see docs/writing.md)*
- [x] P1 🟢 Create a new database (`NEWDATABASE`) *(2CBH)*
- [x] P1 🟢 Read and write PGN *(import is bounded to 2,000 games)*
- [ ] P2 🟢 Export the current search result or selected games as annotated PGN, with progress and a clear choice to include or omit engine comments.
- [ ] P2 🟢 Append games, append EPD, export a selection as text (`APPENDGAMES`, `APPENDEPD`, `CREATETEXTFILE`)
- [x] P2 🟢 Convert between CBH and 2CBH (`CONVERT_TO_FREE_2CBH`, `CONVERTTOCBG`) *(CBH or 2CBH → a new 2CBH: the whole database or up to 2,000,000 selected games. Progress and Stop; checks the target disk's free space against a measured estimate before and while writing. Written to a temporary family and renamed; deleted marks kept; Chess960 games and text entries left out and counted; the source is never touched. 0.4 ms per game on the classic test database. Writing classic CBH is not ported. See [conversion](docs/conversion.md))*
- [ ] P3 🟢 Read CTG/CBOne and older formats (seen in format labels)
- [ ] P3 🟢 Encrypted, password-protected archives (Archive DB dialog) (`DBTOOLSARCHIVE`)

## 2. Database window and maintenance

- [x] P1 🟢 Open a database with its companion files and diagnostics
- [x] P1 🟢 Database start screen: icons, folders, My Databases, recent files (`DBFavorites`, `TOGGLE_FOLDERTREE`, `ADDFOLDERSHORTCUT`) *(My Databases, Recent Databases (16) and folder shortcuts, kept in the app's settings file. Symbols carry the reference app's 20 database types as the workbench's own icons; opening a database adds its symbol, as in the reference app. Remove Database Symbol, Folders, Add Folder Shortcut; Symbols and Details views (Ctrl+I, Ctrl+D). Sizes come from header records only. See [start screen](docs/start-screen.md))*
- [ ] P2 🟢 Database views: Tile, Details, Symbols and Small Symbols (`VIEW_TILE`, `VIEWREPORTEXPL`, `VIEWSYMBOLSEXPL`) *(partial: Symbols and Details on the start screen)*
- [ ] P2 🟢 Database properties: icon, type, description (`EDITDBPROPERTIES`, `Properties`)
- [x] P2 🟢 Several databases open at once *(up to four, a tab each; each keeps its own search, sort and scroll position)*
- [x] P1 🟢 Search Booster: create and delete the fast position index (`DBTOOLSCREATESEARCHBOOSTER`, `DBTOOLSDELETESEARCHBOOSTER`) *(the workbench's own index in its data folder; built by 8 workers, 128 s for the classic test database; the reference app's .cit/.cib files are not read)*
- [x] P2 🟢 Remove deleted games, i.e. compact the database (`DBTOOLSPHYSICALDELETIONS`, `REMOVEDELETEDGAMES`) *(2CBH opened for editing: blocks copied byte for byte into temporary files, swapped in through backups; open games follow their new numbers; the reference app's .2lcd/.2lgd removed for its Check Integrity to rebuild; 0.41 s for the 2CBH test database; see [maintenance](docs/maintenance.md))*
- [x] P2 🟢 Check integrity (`CHECKINTEGRITY`) *(any database, read only: headers, records, name references, every game's moves and annotations, in parallel with progress and Stop; repair and the reference app's control-structure rebuild not ported; see [maintenance](docs/maintenance.md))*
- [ ] P2 🟢 Recover readable games from a damaged database into a new, verified copy, with an itemised report of skipped records; preserve the original files.
- [x] P2 🟢 Find and mark double games, with keep-better and annotation options (`REMOVEDOUBLESCMD`, `DeleteDubs`) *(the reference app's Kill Doubles decision traced (`0x140715720`): annotated games kept, keep-better completeness score, keep different annotators, delete annotated doubles; names Exact/Similar/Ignore, moves Exact/Similar, game length, ignore result/year; marks only, in one write; 2CBH opened for editing, up to 2 M entries; merge/replace, tournaments and clip not ported; see [maintenance](docs/maintenance.md))*
- [ ] P2 🟢 Sort the database, fix sort order (`SORTDBEXPL`, `FIXSORTORDER`) *(the list's sort is a view, as in the reference app; these rewrite the files)*
- [ ] P2 🟢 Set ECO codes, final material, Elo values; reset ratings, teams and rounds (`DBTOOLSSETECOCODE`, `DBTOOLSSETMATERIAL`, `SETELOVALUES`, `RESETRATINGVALUES`)
- [ ] P2 🟢 Improve player and tournament names (`IMPROVEPLAYERNAMES`, `IMPROVETOURNAMENT`, `ImproveDB`): suggest likely duplicates and spelling fixes for players, events, sites and rounds, then preview and approve database-wide changes.
- [ ] P3 🟢 Unannotate a database, cannibalise a database (`UNANNOTATEDB`, `CANNIBALIZEDB`)
- [ ] P3 🟢 Rename, delete and back up database files (`RENAMEDBFILES`, `DBTOOLSDELETEDATABASE`)
- [ ] P3 🟢 Register file types with the OS (`REGISTER_FILE_EXTENSIONS`)
- [ ] P3 🟢 Install a database from DVD or to the hard drive (`INSTALLBIGDB`, `DBTOOLSINSTALL`)

## 3. Game list and list panes

- [x] P1 🟢 Paged game list with White, Black, Elo, result, ECO, event, round and date
- [x] P1 🟢 Deleted and text entries marked
- [x] P1 🟢 Live preview of the selected game *(workbench improvement on the list's Quick Board, `TOGGLEGAMELISTBOARD`)*
- [x] P1 🟢 Load next/previous game (`DBLOADNEXTGAME`, `LOADNEXTGAME`)
- [x] P1 🟢 Go to entry number
- [x] P1 🟢 Virtual scrolling across the whole database instead of 200-entry pages *(blocks of 100 rows read as needed, the list's own scrollbar; Go to entry and Load Next/Previous Game follow the list; the 11.15 M-entry classic test database scrolls end to end)*
- [x] P1 🟢 Sort the list by any column *(header click: ascending, descending, entry order; a sort order per column in the app data folder, built in parallel, cancellable; 0.3–0.8 s on the classic test database warm; search results shown in that order; see docs/game-list.md)*
- [ ] P2 🟢 Configurable columns, list formats and fonts (`SETSELFDEFINEDLISTFORMAT`, `CHOOSEGAMELISTFONT`, `ABBREVIATEFIRSTNAME`)
- [x] P1 🟢 Multi-select, copy, paste and delete games between databases (`DBEDITCOPY`, `DBEDITPASTE`, `DBEDITDELETE`, `DBEDITSELECTALL`) *(Ctrl/Shift-click and Ctrl+A; copy up to 50,000 games in list order; paste into a .2cbh opened for editing with progress and cancel, about 0.5 ms per game; forfeits and 0-0 kept via a CBResult tag; see docs/game-list.md)*
- [x] P2 🟢 Edit game data for a selection: tournament, source, annotator, title (`DBTOOLSEDIT`, `EDITTOURNAMENT`, `EDITSOURCE`, `EDITANNOTATOR`, `EDITGAMETITLE`) *(F2 in the list: tournament, annotator and source for up to 50,000 selected games of a 2CBH opened for editing; only name ids rewritten; progress and Stop; titles and tournament details not ported; see [game list](docs/game-list.md))*
- [x] P2 🟢 Mark games (medals and flags) (`MARKGAME`) *(+ toggles the reference app's user medal, bit 15 of the medal word traced to `0x14013d510`; cyan dot in the list; medals kept by Replace and carried by Paste and conversion; the other fifteen medals are traced (bit order from the Medals dialog, `0x1407c1670`) and named in the list's tooltip; setting them is not ported; see [game list](docs/game-list.md))*
- [ ] P2 🟢 Bookmark a game at its current move, give the bookmark a name or folder, and reopen its database and position from the bookmark list.
- [x] P1 🟢 Players pane with each player's games (`TOGGLETOURNPLAYERS`, `VIEW_GAMES_OF_PLAYER`) *(Players tab: every player alphabetically with games and years, find by name, double-click lists the games by player id; 463,258 players of the classic test database listed in 0.6 s)*
- [x] P1 🟢 Tournaments pane with crosstable (`TOGGLEPANETOURNAMENTS`, `DBTOOLSCROSSTABLE`, `SHOWCROSSTABLE`) *(Tournaments tab with site, games and year; Cross Table (T) and Show Crosstable: round robin or Swiss layout, points, Sonneborn-Berger, Buchholz, wins, performance, category; results open their games; see docs/game-list.md)*
- [ ] P2 🟢 Annotators, sources and teams panes
- [ ] P2 🟢 Opening, theme and endgame keys: classify, insert/transfer keys, key memos (`DBTOOLSCLASSIFYALLKEYS`, `INSERTNEWKEY`, `ASSIGNGAME`, `INSERTMEMOKEY`)
- [ ] P2 🟢 Clipboard database: clip, unclip, remove doubles (`CLIPLOADEDGAME`, `CLIPUNCLIP`, `OPENCLIPBOARD`, `REMOVEDOUBLESFROMCLIP`)
- [ ] P2 🟢 Crosstable tools: tiebreaks, sort, format, ASCII/print (`SELECT_FEINWERTUNG`, `CLIPTABLE`, `PRINTTABLE`, `FORCEROUNDROBBIN`) *(partial: round robin/Swiss switch; tiebreaks fixed to Sonneborn-Berger and Buchholz)*
- [ ] P3 🟢 Replay games automatically (`AUTOMATIC_REPLAY`)
- [ ] P3 🟢 Header and moves list modes (`SETFULLGAMEHEADER`, `SETHEADERANDMOVES`, `SETMOVESONLY`)

## 4. Search

- [x] P1 🟢 Search mask, game data: players, colour, event, site, year, Elo, ECO, result, moves, annotator, source (`OPEN_SEARCHMASK`) *(players with Wins Only and Ignore colours, tournament/site, annotator, Elo modes, Elo difference, year, ECO, moves, result, Mate/Stalemate/Check of the final position and Full Text with NOT; parallel, with progress and Cancel; the game data tab, dialog 311, has no source field; In Repertoire, Time Controls and Text/Good/Beautiful are not ported; see [search](docs/search.md))*
- [x] P1 🟢 Position search: exact position, with OR/NOT boards and mirrored search *(the Position tab, dialog 314, traced in the reference binary: 'Look for', 'Or' and 'Exclude' boards with any-piece and several pieces per square, Horizontal/Vertical mirroring, First/Length/Last, side to move, Include lines; Exact position uses the search booster with mirrored targets; patterns replay games; Sacrifice not ported; on the classic test database a full replay takes 2 min on 8 workers, a boosted search seconds; see [position search](docs/position-search.md))*
- [x] P1 🟢 Find the current board position in a database (`FINDINDBBOARD`, `FINDPOSITION`) *(games open at the position found)*
- [x] P2 🟢 Material search (endgame type, material balance) *(Material tab: editable per-side Q/R/B/N/P/minor/total ranges, endgame presets, signed pawn-unit balance, colour reversal, move window/duration and variations; parallel/cancellable, first matching node; native dialog 328 evidence and explicit differences in [search](docs/search.md#material-search-2026-10-20).)*
- [x] P2 🟢 Annotation and text search: symbols, commentary text, medals (`GENERALSEARCH`) *(the search mask's Annotations tab (dialog 343) and Medals tab (dialog 312), traced from their data exchange: two texts, whole words, symbols, any text or symbol, colours, training, multimedia, pawn structure, piece path, the three critical positions, variations and medals; about 1 s on the classic test database; Deleted, Position and the 960 number not ported; see [search](docs/search.md))*
- [ ] P2 🟢 Manoeuvre search (piece routes) and attack/defence patterns
- [ ] P3 🟢 Game phase and structural theme filters
- [ ] P2 🟢 Save and load search masks (`SAVE_SEARCHMASK`)
- [x] P1 🟢 Quick search expressions: combine player and colour, event/site tags, results, year and ECO ranges, Elo comparisons and excluded terms in the game-list filter. *(Up to 16 terms; quoted values, `!` exclusions, inclusive ranges, average-Elo comparisons and alternative positive results. Invalid expressions fail visibly; the existing indexed scan, cancellation and paging remain. See [search](docs/search.md#quick-filter-expressions).)*
- [x] P2 🟢 Refine the current search result or add matches to it, with a clear choice between searching all games, searching within results and adding to results. *(All / Within / Add in the mask for game-data, material and position searches; immutable result snapshots, sparse refinement, entry-ID union without duplicates, retained node metadata and display sorts, cancellation-safe atomic publication; synthetic tests and live browser UI checks; see [search](docs/search.md#search-all-refine-or-add-2026-10-20).)*
- [ ] P2 🟢 Interactive search preview: show a cancellable, approximate match count while choosing criteria, before the full search runs.
- [ ] P2 🟢 Suggest matching player, event and site names as search criteria are typed.
- [x] P1 🟢 Filter the list from the search mask (`DBTOOLSSEARCH`) *(Ctrl+F searches the whole database; Quick Filter accepts words and combined expressions across the whole database.)*
- [ ] P2 🟢 Search white/black games of a player, games with the same players (`INSTALLWHITESUBSET`, `FIND_PREDECESSORS`)
- [ ] P2 🟢 Similar endgames, structures, moves, tactics and patterns (`SIMILAR_ENDGAMES`, `SIMILAR_STRUCTURES`, `SIMILAR_MOVES`, `AR_TACTICS`, `SimilarPattern`)
- [ ] P2 🟡 Online database search; Lichess's opening explorer API could stand in (`FINDONLINEDBBOARD`, `FindOnlineDB`)
- [ ] P3 🟢 Print search results (`PRINTSEARCHRESULT`)
- [ ] P2 🟢 Rank and filter games by quality: good-game criteria, tactical beauty, average player strength and recency-weighted theoretical value; show sortable quality columns.

## 5. Board and replay

- [x] P1 🟢 2D board with coordinates, legal-move hints and promotion choice *(quiet and capture promotions show a vertical icon column over the destination file without resizing the board; click the dimmed board or press Escape to cancel, see [game window](docs/game-window.md#moving-pieces-with-mouse-or-touch))*
- [x] P1 🟢 Flip board (`VIEWFLIPBOARD`, Ctrl+F)
- [x] P1 🟢 Replay with first/previous/next/last and arrow keys (`GOTOBEGIN`, `GOTOEND`, `RETRACTMOVE`, `EXECMOVE`)
- [ ] P2 🟢 Board design: square colours and textures, piece sets, table (`DESIGNBOARD`, `BOARD2D_BACKGROUND`, `BOARD2D_PIECES`, `DESIGNWOOD`)
- [x] P1 🟢 Draw arrows and coloured squares on the board, stored as annotations *(right button or Alt+left; Shift red, Ctrl yellow; undoable, saved to 2CBH and PGN; Delete Colour Commentary `ANNODELETECOLOR` Ctrl+Alt+Y; see [game window](docs/game-window.md))*
- [x] P2 🟢 Drag-and-drop moves *(mouse and touch on editable game, review and training boards; legal targets, promotion, Chess960 castling, flip and cancellation; Position Setup piece relocation; see [game window](docs/game-window.md#moving-pieces-with-mouse-or-touch))*
- [ ] P2 🟢 Board sounds (`BOARDSOUNDS`)
- [ ] P2 🟢 Show material balance and move tooltips (`NOTASHOWMATERIAL`, `NOTASHOWTOOLTIPS`)
- [ ] P2 🟢 Big replay slider (`REPLAY_SLIDER`)
- [ ] P3 🟢 Threats and engine best move shown as arrows (`TOGGLETHREATS`, `TOGGLEBESTMOVE`)
- [ ] P3 🟢 3D board and its options (`VIEWBOARD3D`, `3DBOARDOPTIONS`, `TOGGLE2D3D`)
- [ ] P3 🟢 DGT electronic board (`TOGGLEDGTBOARD`)
- [ ] P3 🟢 Save the position as an image (`SAVEPOSITION`)

## 6. Notation and game editing

- [x] P1 🟢 Notation with variations, move numbers, comments and NAG symbols
- [x] P1 🟢 Enter legal moves and new variations by clicking
- [x] P1 🟢 Text after move, and NAGs by number (`ANNOPOSTTEXT`)
- [x] P1 🟢 Text before move (`ANNOPRETEXT`) *(Ctrl+Shift+A focuses the editor below the notation)*
- [x] P1 🟢 Annotation symbol palette and keyboard symbols (`SHOWSYMBOLPALETTE`) *(the palette (Ctrl+Alt+S) and the reference app's typed-symbol popups: ! or ? move symbols, = evaluations, $ prefixes, traced to the game window's key handler `0x1409e41a0`. The reference app has no per-symbol shortcut. One symbol per group, as 2CBH stores it; Only move is NAG 8. See [game window](docs/game-window.md#annotation-symbols-from-the-keyboard))*
- [x] P1 🟢 Delete and promote variations; delete remaining or previous moves (`DELETEVARIATION`, `PROMOTEVARIATION`, `DELETEREMAININGMOVES`, `DELETEPREVIOUSMOVES`) *(Ctrl+Y, Alt+Up; Delete Previous Moves starts the game from the current position)*
- [x] P1 🟢 Undo/redo for game edits (`EDIT_UNDO`, `EDIT_REDO`) *(Ctrl+Z, Ctrl+Alt+Shift+Y; the last 100 edits, at most 200,000 stored nodes)*
- [x] P1 🟢 Edit game data: players, event, result, Elo, ECO (`EDITGAMEHEADER`, F2) *(players, ratings, tournament, site, round, date, result, ECO and annotator as PGN tags, checked before they are kept)*
- [x] P1 🟢 New game and position setup (`GAMENEW`, `NEWPOSITION`) *(Ctrl+N; S opens a board editor with side to move, castling, en passant, move number and FEN)*
- [x] P1 🟢 Copy the position as FEN (`EDITCOPYPOSITION`) *(partial: no picture yet)*
- [x] P1 🟢 Copy and paste games as PGN; paste a position from FEN (`EDITCOPYGAME`, `EDITPASTEGAME`, `EDITPASTEPOSITION`) *(Ctrl+C, Ctrl+V; when the clipboard cannot be read, a dialog takes the text)*
- [x] P2 🟢 Null move, exchange moves, insert a variation into the game, reorder or clean up variations (`GAMENULLMOVE`, `EXCHANGEMOVES`, `INSERTVARIATIONINTOGAME`, `REORDERVARIATIONS`, `CLEANUPVARIATIONS`) *(Enter Null Move (Ctrl+Alt+0), Insert Into Game, Exchange Moves (first move of a variation, traced to `0x140b5bc60`), Reorder Variations (largest subtree first, "Including the mainline?", `0x140b62270`) and Clean up variations (lines merged at their deepest known position, `0x1408e6d70`/`0x140b64c80`), each one undoable edit; see [game window](docs/game-window.md))*
- [x] P2 🟢 Fold and unfold variations (`FOLDALL`, `UNFOLDALL`, `FOLDNEXTNODE`) *(Fold/Unfold All, next node with Ctrl+-/Ctrl++, [+n] in the notation; choosing a folded move unfolds; folding the others and switching folding off not ported)*
- [x] P2 🟢 Delete commentary by kind: all, colour, evaluation/time, text, training, multimedia (`ANNODELETEALL`, `ANNODELETECOLOR`, `ANNODELETEFRITZ`) *(all (variations too, Ctrl+Shift+Y), text, evaluation and time (scores written as text and PGN [%eval]/[%clk]/[%emt], Alt+Shift+Y) and colour, each undoable; training and multimedia annotations are not decoded, so there is nothing to delete)*
- [x] P2 🟢 Insert diagrams, set medals, mark critical positions (opening, middlegame, endgame) (`ANNODIAGRAM`, `ANNOMEDAL`, `ANNOCRITICALPOSOPENING`) *(diagram 0x06 and move medal 0x22 decoded from both families, shown and written to 2CBH; Set Medal uses the game medals' 16 bits; critical positions were already done; see [annotations](docs/annotations.md))*
- [x] P2 🟢 Pawn structure and piece path annotations (`ANNOPAWNSTRUCTURE`, `ANNOPIECEPATH`) *(types 0x14 and 0x15 decoded, shown and written: the reference app's value 3 and yellow destination square; the board highlights the pawns or draws the piece's path, which is the workbench's own drawing; see [annotations](docs/annotations.md))*
- [x] P2 🟢 Move times and clock comments (`ANNO_MOVETIME`) *(type 0x07 decoded as hours/minutes/seconds, Move Time (Ctrl+J) edits it, PGN `[%emt]` in and out; stored evaluations (0x21) are shown and kept as PGN `[%eval]`; see [annotations](docs/annotations.md))*
- [ ] P2 🟢 Keyboard move input mode (`GAME_INPUT_MODE`)
- [ ] P3 🟢 Variation colours, fonts and evaluation profile in the notation (`ANNOVARIATIONCOLOR`, `NOTACHOOSEFONT`, `NOTAEVALPROFILE`) *(partial: Show Evaluation Profile done, see [evaluation profile](docs/evaluation-profile.md))*
- [ ] P3 🟢 Game quotations: games embedded inside a game (`ANNOGAMEQUOTATION`)
- [ ] P3 🟢 Compare games or notations side by side (`COMPAREGAMES`, `COMPARENOTATIONS`)
- [ ] P3 🟢 Multimedia annotations: audio, picture, video (`ANNORECORDAUDIO`, `ANNOCOPYPICTURE`, `ANNOCOPYVIDEO`)

## 7. Engines and analysis

- [x] P1 🟢 Local UCI engine: choose, configure options, start/stop analysis (`ADDKIBITZER`, `CREATEUCIENGINE`, Alt+F2)
- [x] P1 🟢 MultiPV lines with depth, nodes, score and SAN principal variation
- [x] P1 🟢 Increase and decrease the number of lines (`INCREASENLINES`, `DECREASENLINES`) *(+ and −: MultiPV, restarting the analysis; see [engines](docs/engines.md))*
- [x] P1 🟢 Paste engine move or variation into the notation (`PASTEENGINEMOVE`, `PASTEENGINEVARIATIONS`, Ctrl+Space) *(Shift+Space for the move; one undoable edit; the score and depth as a text comment because the reference app's evaluation annotation is not decoded)*
- [x] P1 🟢 Engine management: installed engine list, defaults, several kibitzers (`ENGINEMANAGEMENT`, `CLOSEALLKIBITZERS`, `TOGGLEDEFAULTKIBITZER`) *(up to 4 kibitzers; installed engines, default and kept options in the app's settings; Default Kibitzer Alt+F2)*
- [x] P1 🟢 Endgame tablebases (Syzygy) for analysis and the notation *(Tablebase tab with the result and DTZ of the position and every move; engines get the chosen folder through `SyzygyPath`, as the reference app does; Tablebase Annotation marks main-line moves that give away a win or draw, with the tables' best move, as one undoable edit (the workbench's own symbols and texts, since the reference app's notation display was not traced); see [engines](docs/engines.md))*
- [x] P1 🟢 Tactical analysis or blunder check that annotates a game (`TACTICAL_ANALYSIS`, `BlunderCheck`) *(Ctrl+Alt+Shift+U: the reference app's Time and Fine/Medium/Coarse thresholds (35/55/150 cp), cancellable with progress, one undoable edit. Evaluation symbols follow the reference app's traced rule (±/⩲/= by score, set where the evaluation changes). The ?/?? rule stays the workbench's own, since the reference app's was not found; see [engines](docs/engines.md))*
- [x] P1 🟢 Guided game review after local engine analysis: show each side's estimated accuracy and mistake summary, a move-by-move quality timeline, critical positions and better continuations; click any finding to jump to the board. *(Built on Centipawn Analysis; the review reopens without rerunning the engine while the game's main line is unchanged. See [game review](docs/game-review.md).)*
- [x] P2 🟢 Learn from mistakes in a reviewed game: hide the suggested move, let the player retry each critical position, then offer a hint, solution or skip and record which ideas need practice. *(The Centipawn Analysis review opens a board at each eligible critical position, checks the saved engine move without changing the game, and keeps solved/needs-practice results locally per game line and suggested move. See [game review](docs/game-review.md).)*
- [x] P2 🟢 Deep analysis: tree-building position analysis (`DEEP_ANALYSIS`) *(the reference app's traced iterations, time per position, depth, widths and 70-centipawn window, with one engine; cancellable with progress, bounded to 400 nodes; the tree goes in as one undoable edit with the reference app's evaluation symbols and the score as text. Multi-engine roles and move symbol 8 are not ported; see [analysis](docs/analysis.md))*
- [x] P2 🟢 Centipawn and achievement analysis (`CENTIPAWN`, `ACHIEVEMENT_ANALYSIS`) *(Centipawn Analysis (Ctrl+Alt+Shift+Z) traced to `0x140a09480` and ported for the shown game: the reference app's time rule, the opening skipped to the ECO table's depth (at least 10 plies), the Error Report's loss and error value, the reference app's move categories as far as a UCI engine allows, and a summary before the first move, as one undoable edit; about 0.77 s per position with Stockfish at 1 s. The database list version (`0x1403fef00`) annotates and replaces the selected games of a 2CBH opened for editing, stores their evaluation profiles, and keeps the reference app's per-player statistics (Perform, Points, values, Weighted Av, opponents) with its CSV layout; game weights use the workbench's move weights, since the reference app's complexity is not available. Achievement Analysis has no command object registered for kc 184, so there is nothing to port; see [centipawn analysis](docs/centipawn-analysis.md))*
- [x] P2 🟢 Analyse threat, lock engine, next best / search better (`ANALYSETHREAT`, `LOCKENGINE`, `NEXTBEST`, `SEARCHBETTER`) *(also Goto Lock Position and Search Clearly Better, with the reference app's keys; traced to `GameWin` slots and the UCI `go … searchmoves` builder `0x140af7a10`. Threat analyses the position after a null move; Search Better compares scores, since UCI has no `minscore`; Search Clearly Better's 0.50 margin is the workbench's choice. See [engines](docs/engines.md#engine-search-commands-threat-lock-next-best))*
- [x] P2 🟢 Analysis jobs queue (`NEW_ANALYSIS_JOBS`, `ADD_ANALYSIS_JOB`, `OPEN_ANALYSIS_JOBS`) *(also `CREATE_CORR_ANALYSIS_JOBS`; the reference app's Analysis and Deep Analysis job types and minimum times, for games of a 2CBH opened for editing; each result replaces its game ("Merge result with original game"); at most 1,000 jobs, with Stop and progress. The queue lasts for the session: the reference app's jobs file is not read or written; see [analysis](docs/analysis.md))*
- [x] P2 🟢 Evaluation profile graph (`TOGGLEEVPROFILE`) *(Ctrl+Alt+P pane and Show Evaluation Profile above the notation; annotation type 0x26 decoded from both families and written back to 2CBH, the same 177 profiles in both test-database families; clicking selects the move. The scale and the critical-move rule (a loss of 1.00 pawn) are the workbench's. See [evaluation profile](docs/evaluation-profile.md))*
- [x] P2 🟢 Instant background analysis when a game loads, with an evaluation profile that follows the notation and lets the user jump to critical moves. *(The reference app's Instant Analysis (kc 181): `V_IDEPTH` × 35 ms per position and Endless's doubling up to 90 s, traced; the default engine in its own process, cancelled when the game or its main line changes, at most 2,000 plies; evaluations cached for the session and shared with Centipawn Analysis. About 0.12 s per position with Stockfish 17 at 105 ms. See [evaluation profile](docs/evaluation-profile.md))*
- [x] P3 🟢 Engine analysis history: retain score, depth, principal variation and time as the engine revises its choice, with a compact/current view toggle. *(History in the kibitzer pane: one row per change of the best move, with the engine's UCI time, the latest depth, score and line; at most 200 choices; see [engines](docs/engines.md#engine-analysis-history))*
- [ ] P3 🟢 Assisted analysis: engine help on hover (`ASSISTED_ANALYSIS`)
- [ ] P3 🟢 Position measurements: Plan Explorer, piece values, heat maps, mobility, attackers/defenders (`POSITION_INFO`)
- [ ] P3 🟢 Position tutor: classifying moves and motifs (`NOTASHOWPOSTUTOR`)
- [ ] P3 🟡 Cloud and remote engines; a self-hosted engine server could replace them (`CLOUD_ANALYSIS`, `LOAD_CLOUD_ENGINE`)
- [ ] P3 🔴 Shared online analysis and live book (`LETS_CHECK`, `TOGGLE_LIVEBOOK_PANE`); a 🟡 option is the Lichess cloud evaluation API

## 8. Openings and repertoire

- [x] P1 🟢 Opening tree from the user's own databases: moves, frequency, score and Elo (`OPENBOOK`, `TOGGLEBOOKPANE`) *(Create Opening Tree builds it into the app data folder; Book tab and extra pane with N, share, %, =%, Av, Perf and last played; main lines only; journal keeps it current across the workbench's own saves; see docs/opening-tree.md)*
- [x] P1 🟢 Reference database search for the current position (`TOGGLEREFERENCEPANE`, `FINDREFDBBOARD`, `AutoSearchDB`) *(Reference tab and pane follow the board, from the opening tree or Find Position; stale searches are cancelled; the reference database is the open database, a separate reference database setting is not implemented)*
- [x] P1 🟢 Filter reference move statistics by Elo, year and a transparent good-game rule; compare a player's move choices with the full reference set. *(Reference pane: both-player Elo and year bounds, a stated 2200/known-result/40-ply rule, and a searchable player plus colour; exact full-tree baseline alongside filtered and player counts, with a visibly bounded 100,000-game posting scan and cancellation. See [opening tree](docs/opening-tree.md).)*
- [ ] P2 🟢 Reference insights for the current line: popularity over time, recent strong-player variations and likely endgame types.
- [ ] P2 🟢 Switch opening-tree sources between the database, current game-list filter and local books, preserving the position and making each source clear.
- [ ] P2 🟢 Build opening reference statistics from selected games, not only the whole database or a saved search.
- [ ] P2 🟢 Opening books: import games or books, copy lines, save the tree (`BOOKIMPORTGAMES`, `BOOKIMPORTBOOK`, `COPYLINETOBOOK`, `SAVETREE`)
- [x] P2 🟢 Opening classification with ECO and opening keys (`CLASSIFYGAME`, `ClassifyOpening`) *(ECO only, from the open lichess table (CC0) instead of the reference app's Eco.cod: Classify Opening (Ctrl+Alt+C) with Set ECO, and Set ECO Codes (`DBTOOLSSETECOCODE`) for a 2CBH opened for editing, cancellable with progress, keeping existing codes by default. The table gives the reference app's main code for 76 % of the 2CBH test database's coded games. Opening keys (`.cko`) are not ported; see [openings](docs/openings.md))*
- [x] P2 🟢 Opening report from a database (`OpeningReport`) *(the reference app's sections as traced: title with ECO, results, history (earliest/latest annotated/latest strong/latest games, games per year, fashion index in the reference app's bucket sizes), strong and other notable players (the reference app's rule), move rows with "played by" from the tree or the strongest games, and the main line (≥ 3 games); cancellable with progress, at most 100,000 games. Plans, "You should play", critical lines and opening keys are not ported; see [openings](docs/openings.md))*
- [ ] P2 🟢 Repertoire database: add white/black lines, find, report, print (`ADDTOREPERTOIREBOARD`, `OPEN_REPERTOIREDB`, `PRINTREPERTOIRE`)
- [ ] P2 🟢 Opening-tree overlay independent of any one database: save position-based repertoire moves, comments, evaluations and review markers, and show them over whichever tree is open. *(Partial: local White/Black choices, preferred move, short note and separate practice retry status are done. Engine evaluations and full review markers remain future work. See [repertoire workspace](docs/repertoire-workspace.md).)*
- [x] P2 🟢 Generate a repertoire from your own games (`GENERATEREPERTOIRE`) *(both commands traced: Generate Repertoire (`0x14070e970`) writes White and Black databases from the user's games, one game per ECO code played twice, merged to 24 plies, with the player's move where the classification ends marked as a critical opening position; Repertoire Report White/Black (`0x14070f570`) lists the games reaching those marks, best 100 per position. Ported from the open database with the open ECO table into new 2CBH files; codes with 16+ games split by table entry instead of the untraced clustering; the report takes each game's deepest mark, not the first; 1.14 s to generate for 6,925 games and 25–38 s to report over 2,000,000 games warm on the classic test database; critical positions (0x18) now read, written and shown; see [repertoire](docs/repertoire.md))*
- [ ] P2 🟢 Novelty finder and novelty annotation (`FINDNOVELTYBOARD`, `FIND_NOVELTY`) *(partial: Find Novelty (Shift+F6) traced to `0x1407ac5c0` and ported against the open database: the first move no other game reaches, the games before it (at most 400) and the novelty selected; about 0.1 s a game with the opening tree. Novelty Annotation (list) uses an engine rule that was not traced and is not ported; see [novelty](docs/novelty.md))*
- [ ] P3 🟢 Opening survey: generated overview with styles and depths (`OPENING_SURVEY`)
- [ ] P3 🟢 Collect openings, i.e. cluster a database (`CLUSTER_OPENINGS`)
- [ ] P3 🟢 Editorial annotation (RR), reference-style annotation (`EDITORIALANNOTATIONBOARD`)
- [ ] P3 🔴 MyMoves and cloud repertoire sync (`UPLOAD_MYMOVES`, `CLOUD_SYNCREPERTOIRE`)

## 9. Reports and statistics

- [x] P1 🟢 Player statistics: score by colour, opening and year (`STATISTICS`, `PLAYERSTATISTIC`) *(the reference app's Player statistic layout, dialog 846: score by colour and year, then opponents, tournaments and openings by ECO with its sort buttons and Table; performance and average Elo; one cancellable parallel scan, 0.06 s for Carlsen's 6,925 games on the classic test database warm; rows open their games; the graphical Statistics dialog and opening keys are not ported; see [player statistics](docs/player-statistics.md))*
- [x] P2 🟢 Player dossier (`PLAYERDOSSIER`) *(traced: the generator `0x1408358a0` and its sections, statistics tallies and verdict band tables; ported: statistics with the reference app's bands in the workbench's words, years, score by rating band, tournaments, opponents, repertoire for each colour, medal games, mates given and received, self-annotated games; 0.62 s warm for 6,925 games on the classic test database; biography, pictures and rating history need the player encyclopedia and are left out; see [player dossier](docs/player-dossier.md))*
- [x] P2 🟢 Tournament crosstable and report (`DBTOOLSCROSSTABLE`, `TournamentReport`) *(crosstable done, §3; report traced to `0x14083a600`/`0x14083a800`: rounds with date and pairings, round range, two columns; ported with standings after each round and result statistics; reached from Cross table, both ribbons; see [tournament report](docs/tournament-report.md))*
- [x] P2 🟢 Error report: blunder statistics and estimated rating (`BLUNDER_REPORT`) *(traced: Quick/Slower depths 9/12, per-move loss and weights, error value, the Blunder Elo curve `0x1404705d0`; ported with the user's UCI engine: the reference app's curve and error value with the weights a UCI engine allows, plus severity, losses by size, phase, opponent, evaluation transitions and worst moves; within 82–292 points of three strong players' ratings with Stockfish at depth 9; complexity, shape, slope and GM reference charts not ported; see [error report](docs/error-report.md))*
- [ ] P2 🟢 Filter graphs for the current game list: absolute counts and share of the full database by year, rating and game length, with clear handling of missing ratings.
- [ ] P2 🟢 Personal play insights across reviewed games: filter move quality and results by opening, game phase, colour and opponent strength, and open the games behind each result.
- [ ] P3 🟢 Style report (`STYLE_REPORT`) *(Partial native reconstruction: eight dimensions replace the initial descriptive metrics; Score and Decided Games use recovered formulas. Six dimensions and categorical grades remain unavailable because essential native inputs/behavior are not reproduced. Selection, cancellation and supporting navigation are implemented; see [playing style](docs/playing-style.md).)*
- [ ] P3 🟢 Game overview and theme classification (`OVERVIEWBOARD`, `THEMECLASSIFYBOARD`)
- [ ] P3 🟢 Piece probability and piece first-move statistics (`DBTOOLSPIECEPROBABILITY`, `DBTOOLSPIECEFIRSTMOVE`)
- [ ] P3 🟢 Endgame classification summaries: sortable endgame types per game and comparisons between a player's or opening's endgames and the reference database.
- [ ] P3 🟢 Player map (`SHOWMAP`)

## 10. Players and preparation

- [x] P2 🟢 Prepare against White or Black from the database (`PREPARE_WHITE`, `PREPARE_BLACK`, `PREPARE_AGAINST`) *(traced: the player's games of one colour, deleted left out, in a game window with the last game loaded, and the average of the last ten ratings; the workbench shows score, recent rating, repertoire lines with ECO names, openings and latest games, and the player's tree in the Book pane; a player's 3,515 White games read in 0.12 s warm on the classic test database; the window's tree mode and rating formula use not traced; see [preparation](docs/preparation.md))*
- [ ] P2 🟡 Player encyclopedia: names, photos, ratings and biography; FIDE rating lists could supply open data (`OPENPLAYERBASE`, `PB_EDITPLAYER`, `SHOWELOLIST`)
- [ ] P3 🟢 Player photos in the game window (`TOGGLEPHOTOS`)
- [ ] P3 🟡 Find a player online or match Lichess accounts (`SEARCHPLAYER`, `ComparePlayerOnline`)

## 11. Training

- [x] P2 🟢 Training annotations: questions with hints and points (`ANNOTRAINING`, `CREATETRAINING`) *(type 0x09 traced and decoded in CBA and 2CBA: move input or multiple choice, seconds, score, question, wrong-answer text, two hints in 7 languages with board marks, answers with points and feedback; answer squares are 0-based on the position before the move, all 3,687 answers of the classic test database legal; read, written to 2CBA (all 2,304 games round-trip) and shown in the notation; Training Annotation (Ctrl+Alt+M) edits a question one language at a time, Delete Training Commentary removes them; PGN export asks first; asking the questions is Replay Training; see [training](docs/training.md))*
- [x] P2 🟢 Replay training: practise authored move and multiple-choice questions from the current game or selected games, with hints, feedback, retry and local progress. Legal unauthored moves are ungraded; there is no engine verdict. See [Replay Training Sessions](docs/replay-training.md).
- [ ] P2 🟢 Calculation training (`CALCULATION_TRAINING`, `CALC_CHECK_LEGAL`, `CALC_CHECK_ENGINE`)
- [ ] P3 🟢 Tactics training from positions (`TRAINING_TACTICS`, `TACTICS_FIGHT`)
- [ ] P2 🟢 Harvest training positions from selected games with a local engine, creating a reviewable tactics database with questions and progress.
- [ ] P2 🟢 Themed puzzle practice from local training positions, with hints, repeatable sets and a dashboard of accuracy and progress by theme.
- [ ] P3 🟢 Opening and dynamic hints (`OPENING_HINTS`, `DYNAMIC_HINTS`)
- [ ] P3 🟢 Full tree replay of a repertoire (`FullTreeReplay`)
- [ ] P3 🔴 Daily tactics and online play-out (`TOGGLE_DAILYTACTICS`, `PLAY_IN_FRITZONLINE`)

## 12. Text, printing and publishing

- [ ] P2 🟢 Text entries: rich text with game, position and player links (`NEWTEXT`, `GAMELINK`, `POSITIONLINK`, `CCREATETABLE`)
- [ ] P2 🟢 Print a game, diagram or scoresheet, with page setup (`PRINTSINGLEGAME`, `PRINTSINGLEDIAGRAM`, `PRINTSCORESHEET`, `PAGESETUP`)
- [ ] P2 🟢 Diagram lists: new, insert, edit, print, save (`NEWDIAGRAMLIST`, `ADDDIAGRAMTOLIST`, `PRINTDIAGRAMLIST`)
- [ ] P2 🟢 Export HTML with a JavaScript replayer (`GENERATEHTML`, `FILE_HTML`)
- [ ] P3 🟢 Export RTF, plain text and eBook formats (Format/export dialog)
- [ ] P3 🟢 Animated GIF and MP4 of a game (`SAVEANIMGIF`, `SAVEMP4`)
- [ ] P3 🟢 QR code for a game (`EXPORT_GAME_TO_QRCODE`, `COPY_GAME_TO_QRCODE`)
- [ ] P3 🟢 E-mail selected games or databases (`MAILGAMES`, `EMAILSELECTEDDB`)
- [ ] P3 🟢 Speak moves and text comments during replay, and keep notation usable with an active screen reader.
- [ ] P3 🟢 Enter moves and basic game controls by voice.
- [ ] P3 🔴 Publish games online and Chess Media System video (`GENERATESILVERLIGHTREADER`, `STREAMING`, `ENCODER`)

## 13. Correspondence chess

- [ ] P3 🟢 Correspondence header, move and record annotations (`EDITCORRRECORD`, `ANNOCORRMOVE`)
- [ ] P3 🟢 Print correspondence cards and reports (`PRINTCORRMOVE`, `PRINTCORRREPORT`)
- [ ] P3 🟡 Correspondence server games, e.g. ICCF (`OPENICCFGAMES`, `SENDICCFMOVE`)
- [ ] P3 🟢 Correspondence analysis jobs (`CREATE_CORR_ANALYSIS_JOBS`)

## 14. Variants and play

- [ ] P3 🟢 Chess960 and Shuffle Chess games and analysis (`CHESS960`, `SHUFFLECHESS`) *(partial: Chess960 games read, replay, edit, export to PGN and analyse with engines offering `UCI_Chess960`; no new-game dialog for Chess960 start positions yet)*
- [ ] P3 🟢 Other variants: Giveaway, Twin, Pawn chess, Makruk, Ouk Chatrang (`GIVEAWAYCHESS`, `TWINCHESS`, `MAKRUK`)
- [ ] P3 🟢 Play against an engine with a clock, blitz or long games (`NEWGAMEBLITZ`, `NEWGAMETOURNAMENT`, `TOGGLECLOCKPANE`)

## 15. The reference app online services (🔴 replace or leave out)

- [ ] P3 🔴 Online playing server: rooms, broadcasts, tournaments, simuls and ratings (≈150 `PLAYSERVER*`, `TOPRATING*`, `NEW*BROADCAST` commands); 🟡 Lichess broadcasts are an open alternative
- [ ] P3 🔴 Cloud databases (`CLOUD_DATABASES`, `CREATE_CLOUD_DATABASE`, `CLOUD_SHARING`); 🟡 a file sync folder is a local alternative
- [ ] P3 🔴 The vendor's online database (`TOGGLEONLINEDB`)
- [ ] P3 🔴 Engine Cloud, Ducats payments and engine providers (`OPEN_CLOUD_ENGINES`, `CHECK_DUCATS_ACCOUNT`, `PROVIDE_*`)
- [ ] P3 🔴 Account, activation, subscriptions and game updates (`EDIT_ACCOUNT`, `ACTIVATE`, `DAM_*`); 🟡 TWIC downloads could supply game updates (`GETTWIC` is declared)
- [ ] P3 🔴 AI Buddy / ChatGPT assistant (`AIBuddy`); 🟡 an assistant with a user-configured model is an option
- [ ] P3 🔴 Shop, Facebook and website links (`FINDINSHOP`, `POST_TO_FACEBOOK`, `GOINTERNET`)

## 16. Application shell

- [x] P1 🟢 reference-app-style ribbon with tabs, groups and tooltips
- [x] P1 🟢 the reference app default keyboard shortcuts (306 recovered; the implemented subset is bound)
- [x] P1 🟢 White, Blue, Gray and Black themes
- [x] P1 🟢 Command search across all commands *(workbench addition)*
- [x] P1 🟢 Resizable panes with layout presets (`LAYOUTSTANDARD`, `LAYOUTBIGBOARD`, `LAYOUTBIGENGINE`)
- [x] P1 🟢 Multiple game tabs at once, plus a "new board" window (`NEWBOARDWINDOW`) *(each opened game gets a tab, up to Options › Maximum game tabs, 1–8, default 8 as the reference app's `MaxGameWins`; per-tab undo, dirty state and origin; quitting asks per tab; see [game window](docs/game-window.md))*
- [ ] P2 🟢 Save and load layouts (`SAVELAYOUT`, `LOADLAYOUT`)
- [ ] P2 🟢 Options dialog (`SETOPTIONS`) and customisable shortcuts (`CUSTOMIZE`) *(partial: Options (Ctrl+Alt+O) has the Limits page, kept in the app's config folder)*
- [ ] P2 🟢 Navigation history back/forward (`HISTORYBACK`, `HISTORYFORWARD`)
- [ ] P2 🟢 Rapid game-entry mode that carries tournament metadata into the next game and makes variation choices explicit.
- [ ] P3 🟢 Minimise ribbon and quick-access toolbar *(minimise done; no quick-access toolbar)*
- [ ] P3 🟢 Full screen and window arrangements (`VIEWFULLSCREEN`, `WINDOWTOP2VERTICAL`)
- [ ] P3 🟢 Localisation: the reference app ships 10 UI languages; the app needs its own translations
- [ ] P3 🟢 Help and shortcut overview (`HELPHELP`, `VIEWSHORTCUTS`)

## 17. Additional OpenFile workflows

These are independent additions to the workbench. They are not mapped to the
reference app's command table.

- [x] P2 🟢 Opening tree from the current search result, with move statistics for only the matching games, a clear indication when a large result is sampled, and a way back to the full database tree. *(Book pane toggle; reads main lines of up to 50,000 retained hits, shows the scanned and total counts, follows transpositions, and cancels obsolete queries; see [opening tree](docs/opening-tree.md#search-result-tree))*
- [ ] P2 🟢 Autosaved work database and session recovery for unfinished games and analysis, without writing to the user's source database.
- [ ] P2 🟢 Searchable daily game history with periodic recovery snapshots for unsaved analysis.
- [ ] P3 🟢 Import history for each database, with a warning before importing the same source again.
- [ ] P2 🟡 Open a PGN URL as a live collection, refresh it, and merge new moves while preserving local comments and variations.
- [ ] P2 🟢 Import ChessBase CBV archives into a new 2CBH database, with bounded extraction and clear diagnostics for unsupported entries.
- [ ] P2 🟢 Coached practice against a local engine: adjustable or adaptive strength, selected opening or current-position starts, hints and mistake warnings, realistic weaker play, a local rating profile and play history.
- [ ] P2 🟢 Local study notebooks with ordered chapters of games or positions, comments and variations; import and export the chapters as PGN for sharing.
- [ ] P3 🟢 Engine matches from the start position or a set of test positions, with clocks, reversed colours, saved results and adjudication.
- [ ] P3 🟢 Optional move-quality cues while entering a move, from available local opening data or an engine, without revealing them during training by default.
