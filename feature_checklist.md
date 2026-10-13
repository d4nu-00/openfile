# ChessBase 18 feature checklist

This file lists ChessBase 18's feature surface, so the workbench can port it over
time. It is built from the command table (806 `M_KC_*` commands), the ribbon
builders, the shipped message catalogues, dialog templates and earlier format
research. See [docs/ui-shell.md](docs/ui-shell.md) for how these were recovered.
Names in backticks are ChessBase command ids, for traceability.

**Status:** `[x]` done in the workbench · `[ ]` not started · *(partial)* noted inline.

**Priority:** **P1** core database and analysis work · **P2** strong value · **P3** niche.

**Portability:**
* 🟢 Can be built locally from the user's own files.
* 🟡 Needs open data or an open service in place of ChessBase's.
* 🔴 Depends on a ChessBase service or account: replace it or leave it out.

## Suggested order

1. Read the newer 2CBH format, decode CBA annotations and write databases (§1).
2. Search mask and position search with a local index (§4).
3. Opening tree built from the user's databases, and the reference search (§8).
4. Complete notation editing: variation operations, game data, symbols (§6).
5. Multiple game tabs, the clipboard database and save/replace into a database (§2, §3).
6. Engine tools: blunder check, deep analysis, analysis jobs, tablebases (§7).
7. Reports, preparation and training (§9–§11), then publishing (§12).

## 1. Database formats and storage

- [x] P1 🟢 Read classic CBH header records, players (CBP) and tournaments (CBT)
- [x] P1 🟢 Decode classic CBG move streams with variations and setup positions
- [ ] P1 🟢 Decode CBG Chess960, special and unencoded game records *(counted: 893 Chess960 and 11 special games in Mega, no unencoded ones; needs Chess960 castling in chess-core)*
- [x] P1 🟢 Read the newer 2CBH/2CBG format *(records, blocks, standard and set-up move trees, 2LID players and tournaments; all 24,759 Mega games cross-checked against the classic file)*
- [x] P1 🟢 Decode CBA annotations: text, symbols, arrows, coloured squares and training *(classic CBA and 2CBA; training, quotations and other kinds are reported, not shown)*
- [x] P1 🟢 Decode annotators (CBC), sources (CBS), teams (CBE) and other companions *(and the 2LID source and team lists; result codes 3–7; titles and remaining CBJ fields not read)*
- [x] P1 🟢 Write databases: save new games, replace games, mark deleted (`SAVEGAME`, `REPLACEGAME`, `DBLISTSAVEGAMECMD`) *(2CBH only; ChessBase's .2lcd/.2lgd indexes are left to its integrity check; see docs/writing.md)*
- [x] P1 🟢 Create a new database (`NEWDATABASE`) *(2CBH)*
- [x] P1 🟢 Read and write PGN *(import is bounded to 2,000 games)*
- [ ] P2 🟢 Append games, append EPD, export a selection as text (`APPENDGAMES`, `APPENDEPD`, `CREATETEXTFILE`)
- [ ] P2 🟢 Convert between CBH and 2CBH (`CONVERT_TO_FREE_2CBH`, `CONVERTTOCBG`)
- [ ] P3 🟢 Read CTG/CBOne/old ChessBase formats (seen in format labels)
- [ ] P3 🟢 Encrypted, password-protected archives (Archive DB dialog) (`DBTOOLSARCHIVE`)

## 2. Database window and maintenance

- [x] P1 🟢 Open a database with its companion files and diagnostics
- [ ] P1 🟢 Database start screen: icons, folders, My Databases, recent files (`DBFavorites`, `TOGGLE_FOLDERTREE`, `ADDFOLDERSHORTCUT`)
- [ ] P2 🟢 Database views: Tile, Details, Symbols and Small Symbols (`VIEW_TILE`, `VIEWREPORTEXPL`, `VIEWSYMBOLSEXPL`)
- [ ] P2 🟢 Database properties: icon, type, description (`EDITDBPROPERTIES`, `Properties`)
- [ ] P2 🟢 Several databases open at once
- [ ] P1 🟢 Search Booster: create and delete the fast position index (`DBTOOLSCREATESEARCHBOOSTER`, `DBTOOLSDELETESEARCHBOOSTER`)
- [ ] P2 🟢 Remove deleted games, i.e. compact the database (`DBTOOLSPHYSICALDELETIONS`, `REMOVEDELETEDGAMES`)
- [ ] P2 🟢 Check integrity (`CHECKINTEGRITY`)
- [ ] P2 🟢 Find and mark double games, with keep-better and annotation options (`REMOVEDOUBLESCMD`, `DeleteDubs`)
- [ ] P2 🟢 Sort the database, fix sort order (`SORTDBEXPL`, `FIXSORTORDER`)
- [ ] P2 🟢 Set ECO codes, final material, Elo values; reset ratings, teams and rounds (`DBTOOLSSETECOCODE`, `DBTOOLSSETMATERIAL`, `SETELOVALUES`, `RESETRATINGVALUES`)
- [ ] P2 🟢 Improve player and tournament names (`IMPROVEPLAYERNAMES`, `IMPROVETOURNAMENT`, `ImproveDB`)
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
- [ ] P1 🟢 Virtual scrolling across the whole database instead of 200-entry pages
- [ ] P1 🟢 Sort the list by any column
- [ ] P2 🟢 Configurable columns, list formats and fonts (`SETSELFDEFINEDLISTFORMAT`, `CHOOSEGAMELISTFONT`, `ABBREVIATEFIRSTNAME`)
- [ ] P1 🟢 Multi-select, copy, paste and delete games between databases (`DBEDITCOPY`, `DBEDITPASTE`, `DBEDITDELETE`, `DBEDITSELECTALL`)
- [ ] P2 🟢 Edit game data for a selection: tournament, source, annotator, title (`DBTOOLSEDIT`, `EDITTOURNAMENT`, `EDITSOURCE`, `EDITANNOTATOR`, `EDITGAMETITLE`)
- [ ] P2 🟢 Mark games (medals and flags) (`MARKGAME`)
- [ ] P1 🟢 Players pane with each player's games (`TOGGLETOURNPLAYERS`, `VIEW_GAMES_OF_PLAYER`)
- [ ] P1 🟢 Tournaments pane with crosstable (`TOGGLEPANETOURNAMENTS`, `DBTOOLSCROSSTABLE`, `SHOWCROSSTABLE`)
- [ ] P2 🟢 Annotators, sources and teams panes
- [ ] P2 🟢 Opening, theme and endgame keys: classify, insert/transfer keys, key memos (`DBTOOLSCLASSIFYALLKEYS`, `INSERTNEWKEY`, `ASSIGNGAME`, `INSERTMEMOKEY`)
- [ ] P2 🟢 Clipboard database: clip, unclip, remove doubles (`CLIPLOADEDGAME`, `CLIPUNCLIP`, `OPENCLIPBOARD`, `REMOVEDOUBLESFROMCLIP`)
- [ ] P2 🟢 Crosstable tools: tiebreaks, sort, format, ASCII/print (`SELECT_FEINWERTUNG`, `CLIPTABLE`, `PRINTTABLE`, `FORCEROUNDROBBIN`)
- [ ] P3 🟢 Replay games automatically (`AUTOMATIC_REPLAY`)
- [ ] P3 🟢 Header and moves list modes (`SETFULLGAMEHEADER`, `SETHEADERANDMOVES`, `SETMOVESONLY`)

## 4. Search

- [ ] P1 🟢 Search mask, game data: players, colour, event, site, year, Elo, ECO, result, moves, annotator, source (`OPEN_SEARCHMASK`) *(partial: players with Wins Only and Ignore colours, tournament/site, annotator, Elo modes, year, ECO, moves and result search whole databases; source, Elo difference, mate/stalemate/check and text are not yet searchable)*
- [ ] P1 🟢 Position search: exact position, with OR/NOT boards and mirrored search
- [ ] P1 🟢 Find the current board position in a database (`FINDINDBBOARD`, `FINDPOSITION`)
- [ ] P2 🟢 Material search (endgame type, material balance)
- [ ] P2 🟢 Annotation and text search: symbols, commentary text, medals (`GENERALSEARCH`)
- [ ] P2 🟢 Manoeuvre search (piece routes) and attack/defence patterns
- [ ] P3 🟢 Game phase and structural theme filters
- [ ] P2 🟢 Save and load search masks (`SAVE_SEARCHMASK`)
- [x] P1 🟢 Filter the list from the search mask (`DBTOOLSSEARCH`) *(Ctrl+F searches the whole database; the page text filter is kept as Quick Filter)*
- [ ] P2 🟢 Search white/black games of a player, games with the same players (`INSTALLWHITESUBSET`, `FIND_PREDECESSORS`)
- [ ] P2 🟢 Similar endgames, structures, moves, tactics and patterns (`SIMILAR_ENDGAMES`, `SIMILAR_STRUCTURES`, `SIMILAR_MOVES`, `AR_TACTICS`, `SimilarPattern`)
- [ ] P2 🟡 Online database search; Lichess's opening explorer API could stand in (`FINDONLINEDBBOARD`, `FindOnlineDB`)
- [ ] P3 🟢 Print search results (`PRINTSEARCHRESULT`)

## 5. Board and replay

- [x] P1 🟢 2D board with coordinates, legal-move hints and promotion choice
- [x] P1 🟢 Flip board (`VIEWFLIPBOARD`, Ctrl+F)
- [x] P1 🟢 Replay with first/previous/next/last and arrow keys (`GOTOBEGIN`, `GOTOEND`, `RETRACTMOVE`, `EXECMOVE`)
- [ ] P2 🟢 Board design: square colours and textures, piece sets, table (`DESIGNBOARD`, `BOARD2D_BACKGROUND`, `BOARD2D_PIECES`, `DESIGNWOOD`)
- [ ] P1 🟢 Draw arrows and coloured squares on the board, stored as annotations *(partial: decoded marks are drawn and round-trip through PGN; no drawing tool yet)*
- [ ] P2 🟢 Drag-and-drop moves
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
- [x] P1 🟢 Text after move, and NAGs by number (`ANNOPOSTTEXT`) *(partial: no symbol palette yet)*
- [ ] P1 🟢 Text before move (`ANNOPRETEXT`) *(partial: decoded, shown and kept in PGN; no editor yet)*
- [ ] P1 🟢 Annotation symbol palette and keyboard symbols (`SHOWSYMBOLPALETTE`)
- [ ] P1 🟢 Delete and promote variations; delete remaining or previous moves (`DELETEVARIATION`, `PROMOTEVARIATION`, `DELETEREMAININGMOVES`, `DELETEPREVIOUSMOVES`)
- [ ] P1 🟢 Undo/redo for game edits (`EDIT_UNDO`, `EDIT_REDO`)
- [ ] P1 🟢 Edit game data: players, event, result, Elo, ECO (`EDITGAMEHEADER`, F2)
- [ ] P1 🟢 New game and position setup (`GAMENEW`, `NEWPOSITION`)
- [x] P1 🟢 Copy the position as FEN (`EDITCOPYPOSITION`) *(partial: no picture yet)*
- [ ] P1 🟢 Copy and paste games as PGN; paste a position from FEN (`EDITCOPYGAME`, `EDITPASTEGAME`, `EDITPASTEPOSITION`)
- [ ] P2 🟢 Null move, exchange moves, insert a variation into the game, reorder or clean up variations (`GAMENULLMOVE`, `EXCHANGEMOVES`, `INSERTVARIATIONINTOGAME`, `REORDERVARIATIONS`, `CLEANUPVARIATIONS`)
- [ ] P2 🟢 Fold and unfold variations (`FOLDALL`, `UNFOLDALL`, `FOLDNEXTNODE`)
- [ ] P2 🟢 Delete commentary by kind: all, colour, evaluation/time, text, training, multimedia (`ANNODELETEALL`, `ANNODELETECOLOR`, `ANNODELETEFRITZ`)
- [ ] P2 🟢 Insert diagrams, set medals, mark critical positions (opening, middlegame, endgame) (`ANNODIAGRAM`, `ANNOMEDAL`, `ANNOCRITICALPOSOPENING`)
- [ ] P2 🟢 Pawn structure and piece path annotations (`ANNOPAWNSTRUCTURE`, `ANNOPIECEPATH`)
- [ ] P2 🟢 Move times and clock comments (`ANNO_MOVETIME`)
- [ ] P2 🟢 Keyboard move input mode (`GAME_INPUT_MODE`)
- [ ] P3 🟢 Variation colours, fonts and evaluation profile in the notation (`ANNOVARIATIONCOLOR`, `NOTACHOOSEFONT`, `NOTAEVALPROFILE`)
- [ ] P3 🟢 Game quotations: games embedded inside a game (`ANNOGAMEQUOTATION`)
- [ ] P3 🟢 Compare games or notations side by side (`COMPAREGAMES`, `COMPARENOTATIONS`)
- [ ] P3 🟢 Multimedia annotations: audio, picture, video (`ANNORECORDAUDIO`, `ANNOCOPYPICTURE`, `ANNOCOPYVIDEO`)

## 7. Engines and analysis

- [x] P1 🟢 Local UCI engine: choose, configure options, start/stop analysis (`ADDKIBITZER`, `CREATEUCIENGINE`, Alt+F2)
- [x] P1 🟢 MultiPV lines with depth, nodes, score and SAN principal variation
- [ ] P1 🟢 Increase and decrease the number of lines (`INCREASENLINES`, `DECREASENLINES`) *(command shown, not wired)*
- [ ] P1 🟢 Paste engine move or variation into the notation (`PASTEENGINEMOVE`, `PASTEENGINEVARIATIONS`, Ctrl+Space)
- [ ] P1 🟢 Engine management: installed engine list, defaults, several kibitzers (`ENGINEMANAGEMENT`, `CLOSEALLKIBITZERS`, `TOGGLEDEFAULTKIBITZER`)
- [ ] P1 🟢 Endgame tablebases (Syzygy) for analysis and the notation
- [ ] P1 🟢 Tactical analysis or blunder check that annotates a game (`TACTICAL_ANALYSIS`, `BlunderCheck`)
- [ ] P2 🟢 Deep analysis: tree-building position analysis (`DEEP_ANALYSIS`)
- [ ] P2 🟢 Centipawn and achievement analysis (`CENTIPAWN`, `ACHIEVEMENT_ANALYSIS`)
- [ ] P2 🟢 Analyse threat, lock engine, next best / search better (`ANALYSETHREAT`, `LOCKENGINE`, `NEXTBEST`, `SEARCHBETTER`)
- [ ] P2 🟢 Analysis jobs queue (`NEW_ANALYSIS_JOBS`, `ADD_ANALYSIS_JOB`, `OPEN_ANALYSIS_JOBS`)
- [ ] P2 🟢 Evaluation profile graph (`TOGGLEEVPROFILE`)
- [ ] P3 🟢 Assisted analysis: engine help on hover (`ASSISTED_ANALYSIS`)
- [ ] P3 🟢 Position measurements: Plan Explorer, piece values, heat maps, mobility, attackers/defenders (`POSITION_INFO`)
- [ ] P3 🟢 Position tutor: classifying moves and motifs (`NOTASHOWPOSTUTOR`)
- [ ] P3 🟡 Cloud and remote engines; a self-hosted engine server could replace them (`CLOUD_ANALYSIS`, `LOAD_CLOUD_ENGINE`)
- [ ] P3 🔴 Let's Check shared analysis and LiveBook (`LETS_CHECK`, `TOGGLE_LIVEBOOK_PANE`); a 🟡 option is the Lichess cloud evaluation API

## 8. Openings and repertoire

- [ ] P1 🟢 Opening tree from the user's own databases: moves, frequency, score and Elo (`OPENBOOK`, `TOGGLEBOOKPANE`)
- [ ] P1 🟢 Reference database search for the current position (`TOGGLEREFERENCEPANE`, `FINDREFDBBOARD`, `AutoSearchDB`)
- [ ] P2 🟢 Opening books: import games or books, copy lines, save the tree (`BOOKIMPORTGAMES`, `BOOKIMPORTBOOK`, `COPYLINETOBOOK`, `SAVETREE`)
- [ ] P2 🟢 Opening classification with ECO and opening keys (`CLASSIFYGAME`, `ClassifyOpening`)
- [ ] P2 🟢 Opening report from a database (`OpeningReport`)
- [ ] P2 🟢 Repertoire database: add white/black lines, find, report, print (`ADDTOREPERTOIREBOARD`, `OPEN_REPERTOIREDB`, `PRINTREPERTOIRE`)
- [ ] P2 🟢 Generate a repertoire from your own games (`GENERATEREPERTOIRE`)
- [ ] P2 🟢 Novelty finder and novelty annotation (`FINDNOVELTYBOARD`, `FIND_NOVELTY`)
- [ ] P3 🟢 Opening survey: generated overview with styles and depths (`OPENING_SURVEY`)
- [ ] P3 🟢 Collect openings, i.e. cluster a database (`CLUSTER_OPENINGS`)
- [ ] P3 🟢 Editorial annotation (RR), reference-style annotation (`EDITORIALANNOTATIONBOARD`)
- [ ] P3 🔴 MyMoves and cloud repertoire sync (`UPLOAD_MYMOVES`, `CLOUD_SYNCREPERTOIRE`)

## 9. Reports and statistics

- [ ] P1 🟢 Player statistics: score by colour, opening and year (`STATISTICS`, `PLAYERSTATISTIC`)
- [ ] P2 🟢 Player dossier (`PLAYERDOSSIER`)
- [ ] P2 🟢 Tournament crosstable and report (`DBTOOLSCROSSTABLE`, `TournamentReport`)
- [ ] P2 🟢 Error report: blunder statistics and estimated rating (`BLUNDER_REPORT`)
- [ ] P3 🟢 Style report (`STYLE_REPORT`)
- [ ] P3 🟢 Game overview and theme classification (`OVERVIEWBOARD`, `THEMECLASSIFYBOARD`)
- [ ] P3 🟢 Piece probability and piece first-move statistics (`DBTOOLSPIECEPROBABILITY`, `DBTOOLSPIECEFIRSTMOVE`)
- [ ] P3 🟢 Player map (`SHOWMAP`)

## 10. Players and preparation

- [ ] P2 🟢 Prepare against White or Black from the database (`PREPARE_WHITE`, `PREPARE_BLACK`, `PREPARE_AGAINST`)
- [ ] P2 🟡 Player encyclopedia: names, photos, ratings and biography; FIDE rating lists could supply open data (`OPENPLAYERBASE`, `PB_EDITPLAYER`, `SHOWELOLIST`)
- [ ] P3 🟢 Player photos in the game window (`TOGGLEPHOTOS`)
- [ ] P3 🟡 Find a player online or match Lichess accounts (`SEARCHPLAYER`, `ComparePlayerOnline`)

## 11. Training

- [ ] P2 🟢 Training annotations: questions with hints and points (`ANNOTRAINING`, `CREATETRAINING`)
- [ ] P2 🟢 Replay training: guess the move, graded by engine or theory (ReplayTraining messages)
- [ ] P2 🟢 Calculation training (`CALCULATION_TRAINING`, `CALC_CHECK_LEGAL`, `CALC_CHECK_ENGINE`)
- [ ] P3 🟢 Tactics training from positions (`TRAINING_TACTICS`, `TACTICS_FIGHT`)
- [ ] P3 🟢 Opening and dynamic hints (`OPENING_HINTS`, `DYNAMIC_HINTS`)
- [ ] P3 🟢 Full tree replay of a repertoire (`FullTreeReplay`)
- [ ] P3 🔴 Daily tactics and Fritz online play-out (`TOGGLE_DAILYTACTICS`, `PLAY_IN_FRITZONLINE`)

## 12. Text, printing and publishing

- [ ] P2 🟢 Text entries: rich text with game, position and player links (`NEWTEXT`, `GAMELINK`, `POSITIONLINK`, `CCREATETABLE`)
- [ ] P2 🟢 Print a game, diagram or scoresheet, with page setup (`PRINTSINGLEGAME`, `PRINTSINGLEDIAGRAM`, `PRINTSCORESHEET`, `PAGESETUP`)
- [ ] P2 🟢 Diagram lists: new, insert, edit, print, save (`NEWDIAGRAMLIST`, `ADDDIAGRAMTOLIST`, `PRINTDIAGRAMLIST`)
- [ ] P2 🟢 Export HTML with a JavaScript replayer (`GENERATEHTML`, `FILE_HTML`)
- [ ] P3 🟢 Export RTF, plain text and eBook formats (Format/export dialog)
- [ ] P3 🟢 Animated GIF and MP4 of a game (`SAVEANIMGIF`, `SAVEMP4`)
- [ ] P3 🟢 QR code for a game (`EXPORT_GAME_TO_QRCODE`, `COPY_GAME_TO_QRCODE`)
- [ ] P3 🟢 E-mail selected games or databases (`MAILGAMES`, `EMAILSELECTEDDB`)
- [ ] P3 🔴 Publish games online and Chess Media System video (`GENERATESILVERLIGHTREADER`, `STREAMING`, `ENCODER`)

## 13. Correspondence chess

- [ ] P3 🟢 Correspondence header, move and record annotations (`EDITCORRRECORD`, `ANNOCORRMOVE`)
- [ ] P3 🟢 Print correspondence cards and reports (`PRINTCORRMOVE`, `PRINTCORRREPORT`)
- [ ] P3 🟡 Correspondence server games, e.g. ICCF (`OPENICCFGAMES`, `SENDICCFMOVE`)
- [ ] P3 🟢 Correspondence analysis jobs (`CREATE_CORR_ANALYSIS_JOBS`)

## 14. Variants and play

- [ ] P3 🟢 Chess960 and Shuffle Chess games and analysis (`CHESS960`, `SHUFFLECHESS`)
- [ ] P3 🟢 Other variants: Giveaway, Twin, Pawn chess, Makruk, Ouk Chatrang (`GIVEAWAYCHESS`, `TWINCHESS`, `MAKRUK`)
- [ ] P3 🟢 Play against an engine with a clock, blitz or long games (`NEWGAMEBLITZ`, `NEWGAMETOURNAMENT`, `TOGGLECLOCKPANE`)

## 15. ChessBase online services (🔴 replace or leave out)

- [ ] P3 🔴 Playchess server: rooms, broadcasts, tournaments, simuls and ratings (≈150 `PLAYSERVER*`, `TOPRATING*`, `NEW*BROADCAST` commands); 🟡 Lichess broadcasts are an open alternative
- [ ] P3 🔴 ChessBase Cloud databases (`CLOUD_DATABASES`, `CREATE_CLOUD_DATABASE`, `CLOUD_SHARING`); 🟡 a file sync folder is a local alternative
- [ ] P3 🔴 Online database at database.chessbase.com (`TOGGLEONLINEDB`)
- [ ] P3 🔴 Engine Cloud, Ducats payments and engine providers (`OPEN_CLOUD_ENGINES`, `CHECK_DUCATS_ACCOUNT`, `PROVIDE_*`)
- [ ] P3 🔴 Account, activation, subscriptions and game updates (`EDIT_ACCOUNT`, `ACTIVATE`, `DAM_*`); 🟡 TWIC downloads could supply game updates (`GETTWIC` is declared)
- [ ] P3 🔴 AI Buddy / ChatGPT assistant (`AIBuddy`); 🟡 an assistant with a user-configured model is an option
- [ ] P3 🔴 Shop, Facebook and website links (`FINDINSHOP`, `POST_TO_FACEBOOK`, `GOINTERNET`)

## 16. Application shell

- [x] P1 🟢 ChessBase-style ribbon with tabs, groups and tooltips
- [x] P1 🟢 ChessBase default keyboard shortcuts (306 recovered; the implemented subset is bound)
- [x] P1 🟢 White, Blue, Gray and Black themes
- [x] P1 🟢 Command search across all commands *(workbench addition)*
- [x] P1 🟢 Resizable panes with layout presets (`LAYOUTSTANDARD`, `LAYOUTBIGBOARD`, `LAYOUTBIGENGINE`)
- [ ] P1 🟢 Multiple game tabs at once, plus a "new board" window (`NEWBOARDWINDOW`)
- [ ] P2 🟢 Save and load layouts (`SAVELAYOUT`, `LOADLAYOUT`)
- [ ] P2 🟢 Options dialog (`SETOPTIONS`) and customisable shortcuts (`CUSTOMIZE`)
- [ ] P2 🟢 Navigation history back/forward (`HISTORYBACK`, `HISTORYFORWARD`)
- [ ] P3 🟢 Minimise ribbon and quick-access toolbar *(minimise done; no quick-access toolbar)*
- [ ] P3 🟢 Full screen and window arrangements (`VIEWFULLSCREEN`, `WINDOWTOP2VERTICAL`)
- [ ] P3 🟢 Localisation: ChessBase ships 10 UI languages; the app needs its own translations
- [ ] P3 🟢 Help and shortcut overview (`HELPHELP`, `VIEWSHORTCUTS`)
