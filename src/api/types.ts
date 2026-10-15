export type NodeId = number;
export type Status =
  | "available"
  | "missing"
  | "unsupported"
  | "corrupt"
  | "notDecoded"
  | "partial";
export interface Diagnostic {
  code: string;
  message: string;
  file?: string;
  offset?: string;
}
export interface EntrySummary {
  ordinal: string;
  kind: "game" | "text" | "unknown";
  deleted: boolean;
  /** Marked with the user medal (Mark Games), shown in cyan. */
  marked?: boolean;
  /** ChessBase's other medals (Best game, Novelty, Endgame, …). */
  medals?: string[];
  white?: string;
  black?: string;
  event?: string;
  site?: string;
  date?: string;
  result?: string;
  round?: string;
  whiteElo?: number;
  blackElo?: number;
  eco?: string;
  annotator?: string;
  /** Source title. */
  source?: string;
  whiteTeam?: string;
  blackTeam?: string;
  /** In a position search: the node where the game reaches the position. */
  foundNode?: number;
  /** For the crosstable of the game's tournament. */
  tournamentId?: number;
  /** For the statistics of the game's players. */
  whiteId?: number;
  blackId?: number;
  diagnostics: Diagnostic[];
}
export interface GameNode {
  id: NodeId;
  parent?: NodeId;
  uci?: string;
  san?: string;
  children: NodeId[];
  comments: string[];
  nags: number[];
  /** Text shown before the move. */
  preComments?: string[];
  squares?: SquareMark[];
  arrows?: ArrowMark[];
}
/** Colours use the PGN [%csl]/[%cal] letters: G, Y, R, B, C, O. */
export interface SquareMark {
  color: string;
  square: string;
}
export interface ArrowMark {
  color: string;
  from: string;
  to: string;
}
export interface GameView {
  tags: Record<string, string>;
  startFen: string;
  nodes: GameNode[];
}
export interface OpenDatabaseResponse {
  requestId: string;
  databaseId: string;
  /** The file name without its extension. */
  name?: string;
  entryCount: string;
  companions: { extension: string; status: Status }[];
  diagnostics: Diagnostic[];
  /** Opened for editing: games can be saved, replaced and marked deleted. */
  writable?: boolean;
  /** A current search booster exists for fast position search. */
  booster?: boolean;
  /** A current opening tree exists. */
  tree?: boolean;
}
export interface SaveGameResponse {
  databaseId: string;
  ordinal: string;
  entryCount: string;
  savedRevision: string;
}
/** A game list column the list can be sorted by. */
export type SortColumn =
  | "white"
  | "whiteElo"
  | "black"
  | "blackElo"
  | "result"
  | "eco"
  | "tournament"
  | "round"
  | "date"
  | "annotator"
  | "source"
  | "moves";
export interface SortProgress {
  stage: "names" | "reading" | "sorting";
  done: string;
  total: string;
}
export interface SortResponse {
  /** Rows in the list now. */
  rows: string;
  cancelled: boolean;
  column?: SortColumn;
  descending: boolean;
}
export interface PageResponse {
  requestId: string;
  entries: EntrySummary[];
  next?: string;
}
export interface GameResponse {
  requestId: string;
  documentId?: string;
  game?: GameView;
  revision?: string;
  dirty?: boolean;
  summary: EntrySummary;
  metadataStatus: Status;
  movesStatus: Status;
  annotationsStatus: Status;
  diagnostics: Diagnostic[];
}
export interface PositionResponse {
  requestId: string;
  documentId: string;
  nodeId: NodeId;
  fen: string;
  turn: "white" | "black";
  legalUci: string[];
}
export interface WireError {
  code: string;
  message: string;
  diagnostics: Diagnostic[];
}

export type Edit =
  | { kind: "appendMove"; parent: NodeId; uci: string }
  | { kind: "setComments"; node: NodeId; comments: string[] }
  | { kind: "setNags"; node: NodeId; nags: number[] }
  | { kind: "setPreComments"; node: NodeId; comments: string[] }
  | { kind: "deleteVariation"; node: NodeId }
  | { kind: "promoteVariation"; node: NodeId }
  | { kind: "deleteRemaining"; node: NodeId }
  | { kind: "deletePrevious"; node: NodeId }
  | { kind: "setTags"; tags: Record<string, string> }
  | {
      kind: "setMarks";
      node: NodeId;
      squares: SquareMark[];
      arrows: ArrowMark[];
    }
  | { kind: "clearMarks" }
  | { kind: "insertVariation"; node: NodeId }
  | { kind: "exchangeMoves"; node: NodeId }
  | { kind: "reorderVariations"; node: NodeId; mainLine: boolean }
  | { kind: "cleanUpVariations" }
  | {
      kind: "deleteCommentary";
      commentary: "all" | "text" | "evaluationAndTime";
      node: NodeId;
    }
  | {
      kind: "addAnalysis";
      items: {
        node: NodeId;
        nag?: number;
        evaluation?: number;
        comment?: string;
        line: string[];
        lineComment?: string;
      }[];
    }
  | { kind: "appendLine"; parent: NodeId; moves: string[]; comment?: string }
  | { kind: "addLines"; parent: NodeId; lines: LineItem[] }
  | { kind: "undo" }
  | { kind: "redo" };
/** Edits that change the tree's shape; the selection is re-read after them. */
export const structuralEdits: Edit["kind"][] = [
  "insertVariation",
  "exchangeMoves",
  "reorderVariations",
  "cleanUpVariations",
  "deleteCommentary",
  "deleteVariation",
  "promoteVariation",
  "deleteRemaining",
  "deletePrevious",
  "undo",
  "redo",
];
export interface DocumentResponse {
  documentId: string;
  requestId?: string;
  game: GameView;
  revision: string;
  dirty: boolean;
  /** The node to select after a structural edit. */
  selected?: NodeId;
}
export interface PgnCollectionResponse {
  requestId: string;
  collectionId: string;
  gameCount: number;
  games: Record<string, string>[];
}
export interface SaveResponse {
  savedRevision: string;
  destination: string;
}
export type Color = "white" | "black";
export type ScoreDto =
  | { kind: "cp"; value: number; bound?: string | null }
  | { kind: "mate"; moves: number; bound?: string | null };
export interface EngineOption {
  name: string;
  kind: "check" | "spin" | "combo" | "string" | "button";
  default?: string | null;
  min?: number | null;
  max?: number | null;
  vars: string[];
}
export interface EngineResponse {
  engineId: string;
  identity: Record<string, string>;
  options: EngineOption[];
  status: string;
}
export interface AnalysisInfoDto {
  kind: "info";
  depth?: number;
  nodes?: string;
  multipv: number;
  score?: ScoreDto;
  rootTurn: Color;
  pv?: string[];
  rawPv?: string[];
  diagnostics: string[];
}
export interface EngineEventDto {
  engineId: string;
  requestId: string;
  payload:
    | AnalysisInfoDto
    | { kind: "bestMove"; best: string; ponder?: string }
    | { kind: "error" | "status"; message: string };
}

/** True when a loaded game omits some of its original database annotations. */
export function annotationsIncomplete(status: Status | undefined): boolean {
  return (
    status === "notDecoded" ||
    status === "partial" ||
    status === "corrupt" ||
    status === "unsupported"
  );
}
/** A database (and optionally an entry) named on the command line. */
export interface StartupRequest {
  path?: string;
  entry?: string;
  node?: number | null;
  /** Debug builds: a command id to run once the entry is shown. */
  command?: string;
  /** Debug builds: a column to sort the list by, e.g. "date:desc". */
  sort?: string;
  /** Debug builds: a new game from this position. */
  fen?: string;
}
/** Search mask, game data tab (see docs/search.md). */
export interface PlayerQuery {
  last: string;
  first: string;
}
export type EloMode = "none" | "one" | "both" | "average";
export interface GameFilter {
  white: PlayerQuery;
  black: PlayerQuery;
  ignoreColors: boolean;
  winsOnly: boolean;
  tournament: string;
  annotator: string;
  eloMin?: number;
  eloMax?: number;
  eloMode: EloMode;
  /** At most this rating difference between the players. */
  eloDifference?: number;
  yearMin?: number;
  yearMax?: number;
  ecoFrom: string;
  ecoTo: string;
  movesMin?: number;
  movesMax?: number;
  results: string[];
  excludeDeleted: boolean;
  /** Quick filter: words matched against names, ECO, year and result. */
  text?: string;
  /** Games of one player (players pane). */
  playerId?: number;
  /** Games of one tournament (tournaments pane). */
  tournamentId?: number;
  /** The main line ends in mate, stalemate or check; any of those set. */
  mate?: boolean;
  stalemate?: boolean;
  check?: boolean;
  /** Text in the game's text annotations; with `fullTextNot`, games
   * without it. */
  fullText?: string;
  fullTextNot?: boolean;
}
/** The search mask's position tab (see docs/position-search.md). Pieces
 * are [square, letter]: KQRBNP White, kqrbnp Black, X/x any piece. */
export interface PositionQuery {
  lookFor: [string, string][];
  or: [string, string][];
  exclude: [string, string][];
  /** The 'Look for' board is a whole position. */
  exact: boolean;
  /** Side to move; absent for either (patterns). */
  whiteToMove?: boolean;
  horizontal: boolean;
  vertical: boolean;
  first?: number;
  last?: number;
  length?: number;
  /** Patterns: search variations too. */
  lines: boolean;
}
export type NameKind = "players" | "tournaments";
export interface NameRow {
  id: number;
  name?: string;
  /** Tournaments: the site. */
  site?: string;
  games: number;
  firstYear: number;
  lastYear: number;
}
export interface NamePage {
  requestId: string;
  rows: NameRow[];
}
export interface CrossGame {
  ordinal: number;
  mark: string;
  white: boolean;
  round: number;
  opponent: number;
}
export interface CrossPlayer {
  id: number;
  name?: string;
  elo?: number;
  points: number;
  games: number;
  wins: number;
  blackGames: number;
  blackWins: number;
  sonnebornBerger: number;
  buchholz: number;
  performance?: number;
  cells: CrossGame[][];
}
export type CrossLayout = "roundRobin" | "swiss";
export interface Crosstable {
  event?: string;
  site?: string;
  layout: CrossLayout;
  rounds: number;
  players: CrossPlayer[];
  games: number;
  unscored: number;
  truncated: boolean;
  averageElo?: number;
  category?: number;
}
/** A player's score in a group of games (see docs/player-statistics.md). */
export interface StatScore {
  /** Games with a result. */
  games: number;
  wins: number;
  draws: number;
  losses: number;
  points: number;
  percent?: number;
  averageElo?: number;
  performance?: number;
}
export interface OpeningStat {
  eco?: string;
  white: StatScore;
  black: StatScore;
}
export interface OpponentStat {
  id: number;
  name?: string;
  elo?: number;
  score: StatScore;
  lastYear: number;
}
export interface TournamentStat {
  id: number;
  name?: string;
  site?: string;
  year: number;
  score: StatScore;
}
export interface YearStat {
  year: number;
  score: StatScore;
  elo?: number;
}
export interface PlayerStatistics {
  playerId: number;
  name?: string;
  total: StatScore;
  white: StatScore;
  black: StatScore;
  found: number;
  deleted: number;
  unscored: number;
  firstYear: number;
  lastYear: number;
  openings: OpeningStat[];
  opponents: OpponentStat[];
  tournaments: TournamentStat[];
  years: YearStat[];
  truncated: boolean;
  moreOpponents: number;
  moreTournaments: number;
}
/** Prepare against White or Black: the player's games with one colour. */
export interface RepertoireRow {
  /** UCI moves from the start position. */
  moves: string[];
  /** "1.e4 c5 2.Nf3". */
  text: string;
  eco?: string;
  name?: string;
  games: number;
  /** The player's score in percent. */
  score?: number;
  lastYear?: number;
}
export interface PreparedGame {
  ordinal: string;
  year?: number;
  opponent?: string;
  opponentElo?: number;
  result?: string;
  eco?: string;
  tournament?: string;
}
export interface Preparation {
  playerId: number;
  name?: string;
  white: boolean;
  games: number;
  deleted: number;
  read: number;
  unreadable: number;
  truncated: boolean;
  score: StatScore;
  recentRating?: number;
  firstYear: number;
  lastYear: number;
  repertoire: RepertoireRow[];
  openings: { eco?: string; score: StatScore }[];
  latest: PreparedGame[];
  lastGame?: string;
}
export interface PrepareProgress {
  stage: "games" | "lines";
  done: string;
  total: string;
}
/** The games of a statistics row. */
export type StatGroup =
  | { kind: "all" }
  | { kind: "color"; white: boolean }
  | { kind: "opening"; eco: number; white?: boolean }
  | { kind: "opponent"; id: number }
  | { kind: "tournament"; id: number }
  | { kind: "year"; year: number };
export const emptyFilter = (): GameFilter => ({
  white: { last: "", first: "" },
  black: { last: "", first: "" },
  ignoreColors: false,
  winsOnly: false,
  tournament: "",
  annotator: "",
  eloMode: "none",
  ecoFrom: "",
  ecoTo: "",
  results: [],
  excludeDeleted: false,
});
export interface SearchResponse {
  requestId: string;
  hits: string;
  scanned: string;
  truncated: boolean;
  cancelled: boolean;
  /** A position search used the search booster. */
  indexed?: boolean;
}
export interface BoosterProgress {
  done: string;
  total: string;
}
/** A position as a line: the start position and the moves from it. */
export interface Line {
  startFen: string;
  moves: string[];
}
export type TreeStatus = "tree" | "line" | "notInTree" | "tooMany" | "noTree";
export interface TreeMove {
  uci: string;
  san: string;
  games: number;
  /** Percentage of the position's games. */
  share: number;
  /** Score of the side to move, in percent. */
  score?: number | null;
  draws?: number | null;
  averageElo?: number | null;
  performance?: number | null;
  firstYear?: number | null;
  lastYear?: number | null;
}
export interface TreeResponse {
  requestId: string;
  status: TreeStatus;
  games: number;
  /** Games that end in the position. */
  ended: number;
  moves: TreeMove[];
}
export interface TreeProgress {
  stage: "reading" | "sorting";
  done: string;
  total: string;
}
export type TablebaseOutcome =
  | "win"
  | "maybeWin"
  | "cursedWin"
  | "draw"
  | "blessedLoss"
  | "maybeLoss"
  | "loss";
/** Syzygy tablebases for a position, from the side to move. */
export interface TablebaseResponse {
  requestId: string;
  status:
    "noTables" | "tooManyPieces" | "castling" | "missing" | "failed" | "found";
  message?: string;
  pieces: number;
  maxPieces: number;
  outcome?: TablebaseOutcome;
  dtz?: number;
  rounded: boolean;
  /** Best first; each outcome is for the side that plays the move. */
  moves: {
    uci: string;
    san: string;
    outcome: TablebaseOutcome;
    dtz?: number;
    rounded: boolean;
    zeroing: boolean;
    ends?: "mate" | "stalemate";
  }[];
}
export interface BlunderParams {
  engine: string;
  options: Record<string, string>;
  millisPerMove: number;
  /** The smallest loss that counts, in centipawns. */
  threshold: number;
  side: "white" | "black" | "both";
}
export interface BlunderFinding {
  node: NodeId;
  loss: number;
  best: string;
  played: string;
  depth?: number | null;
  line: string[];
}
export interface BlunderReport {
  cancelled: boolean;
  checked: number;
  findings: BlunderFinding[];
  /** ChessBase's evaluation symbols where the evaluation changes. */
  evaluations: { node: number; nag: number }[];
  revision: string;
}
/** One line of an analysis result, from the analysed position. */
export interface LineItem {
  moves: string[];
  /** ChessBase's evaluation symbol for the last move. */
  evaluation?: number;
  /** The score and depth as text, e.g. "+0.35/20". */
  comment?: string;
}
export interface DeepParams {
  engine: string;
  options: Record<string, string>;
  node: NodeId;
  /** 0 runs until stopped. */
  timeLimitSecs: number;
}
export interface DeepProgress {
  iteration: number;
  positions: number;
  nodes: number;
  elapsedMs: number;
  /** The best line in SAN and its score from White's side. */
  best: string[];
  score?: string;
}
export interface DeepReport {
  cancelled: boolean;
  iterations: number;
  positions: number;
  /** The tree reached its node bound. */
  limited: boolean;
  node: NodeId;
  lines: LineItem[];
  revision: string;
  best: string[];
  score?: string;
}
/** ChessBase's job types: Analysis (1) and Deep Analysis (2). */
export type JobKind =
  | { type: "analysis"; lines: number; seconds: number }
  | { type: "deep"; seconds: number };
export type JobStatus = "waiting" | "running" | "done" | "skipped" | "failed";
export interface AnalysisJob {
  id: number;
  databaseId: string;
  ordinal: string;
  label: string;
  moves: string[];
  kind: JobKind;
  status: JobStatus;
  message?: string;
}
export interface JobsView {
  jobs: AnalysisJob[];
  running: boolean;
}
export interface JobsProgress {
  job?: number;
  done: number;
  total: number;
  jobs: AnalysisJob[];
}
export interface RunJobsParams {
  engine: string;
  options: Record<string, string>;
}
export interface RunJobsReport {
  stopped: boolean;
  done: number;
  skipped: number;
  failed: number;
  jobs: AnalysisJob[];
}
/** Classify Opening: the shown game's opening from the open table. */
export interface Classification {
  eco?: string;
  name?: string;
  /** The main-line ply whose position the table holds. */
  ply?: number;
  /** The game's own ECO tag. */
  current?: string;
}
export interface EcoResult {
  games: string;
  changed: string;
  unchanged: string;
  kept: string;
  unclassified: string;
  unreadable: string;
  agreed: string;
  hadCode: string;
  cancelled: boolean;
}
/** A game in the opening report. */
export interface ReportGame {
  ordinal: string;
  white?: string;
  black?: string;
  whiteElo?: number;
  blackElo?: number;
  date?: string;
  result?: string;
  event?: string;
}
export interface ReportPlayer {
  name: string;
  games: number;
  /** The player's score in these games, in percent. */
  score: number;
  bestElo?: number;
  averageElo?: number;
}
export interface ReportMove {
  uci: string;
  san: string;
  games: number;
  share: number;
  score?: number;
  draws?: number;
  averageElo?: number;
  performance?: number;
  lastYear?: number;
  playedBy: string[];
}
export interface OpeningReport {
  eco?: string;
  name?: string;
  line: string[];
  whiteToMove: boolean;
  games: number;
  truncated: boolean;
  cancelled: boolean;
  whiteWins: number;
  draws: number;
  blackWins: number;
  otherResults: number;
  listed: ReportGame[];
  earliest?: ReportGame;
  latestAnnotated?: ReportGame;
  latestStrong?: ReportGame;
  latest?: ReportGame;
  years: [number, number][];
  fashion: { from: number; to: number; games: number; index?: number }[];
  strongWhite: ReportPlayer[];
  strongBlack: ReportPlayer[];
  notable: ReportPlayer[];
  moves: ReportMove[];
  movesFromTree: boolean;
  read: number;
  mainLine: string[];
}
/** An engine in Engine Management. */
export interface InstalledEngine {
  id: string;
  name: string;
  path: string;
  /** Options changed from the engine's defaults. */
  options: Record<string, string>;
}
/** The workbench's settings, kept in its config folder. */
export interface AppSettings {
  /** ChessBase's "Maximum Board Windows": 1 to 8, 8 by default. */
  maxGameTabs: number;
  engines: InstalledEngine[];
  defaultEngine?: string;
  /** The folder of Syzygy tablebase files. */
  tablebaseDir?: string;
}
export const defaultSettings: AppSettings = { maxGameTabs: 8, engines: [] };
export interface ReferenceResponse {
  requestId: string;
  hits: string;
  truncated: boolean;
  cancelled: boolean;
  /** `notSearched`: the tree cannot answer and Find Position would read
   * every game of a large database without a search booster. */
  source: "tree" | "line" | "search" | "notSearched";
  /** A fallback search used the search booster. */
  indexed: boolean;
}
/** The names Edit Game Data sets for a selection; a field left out is
 * kept, an empty one cleared. */
export interface SelectionNames {
  /** Site and event. */
  tournament?: [string, string];
  annotator?: string;
  /** Title and publisher. */
  source?: [string, string];
}
export interface SelectionEditResponse {
  changed: string;
  /** Mark Games: games marked afterwards. */
  marked: string;
  cancelled: boolean;
}
/** One problem Check Integrity found; entry 0 is a problem of the files. */
export interface IntegrityProblem {
  ordinal: number;
  code: string;
  message: string;
}
export interface IntegrityReport {
  checked: number;
  games: number;
  texts: number;
  deleted: number;
  counts: Record<string, number>;
  problems: IntegrityProblem[];
  cancelled: boolean;
}
export type DoubleCompare = "exact" | "similar" | "ignore";
/** ChessBase's Kill Doubles options that the workbench ports. */
export interface DoubleOptions {
  decision: "first" | "second" | "keepBetter";
  names: DoubleCompare;
  moves: DoubleCompare;
  minMoves: number;
  ignoreResult: boolean;
  ignoreYear: boolean;
  keepAnnotators: boolean;
  deleteAnnotated: boolean;
}
export interface DoublesResponse {
  checked: string;
  marked: string;
  keptBoth: string;
  /** The first pairs, as (marked, kept) entry numbers. */
  pairs: [string, string][];
  cancelled: boolean;
  treeStale: boolean;
}
export interface CompactResponse {
  kept: string;
  removed: string;
  bytesBefore: string;
  bytesAfter: string;
  cancelled: boolean;
  entryCount: string;
  /** Open games whose entry number changed, as (old, new); new "0" when removed. */
  moved: [string, string][];
  treeDeleted: boolean;
}
/** The space Convert to 2CBH needs, against the target disk's free space. */
export interface ConvertEstimate {
  games: string;
  bytes: string;
  free?: string | null;
  fits: boolean;
}
export interface ConvertResponse {
  /** Entries looked at, games written and kept deleted. */
  done: number;
  written: number;
  deleted: number;
  /** Games written without some annotations. */
  incomplete: number;
  /** Entries left out: texts, Chess960 games, unreadable moves. */
  texts: number;
  chess960: number;
  unreadable: number;
  cancelled: boolean;
  /** The new database, when it was written. */
  path?: string | null;
}
export interface PasteResponse {
  pasted: string;
  /** Entries that are not games, or whose moves could not be read. */
  skipped: string;
  /** Games stored without some annotations. */
  incomplete: string;
  cancelled: boolean;
  entryCount: string;
  /** The opening tree no longer covers the database. */
  treeStale: boolean;
  /** Chess960 games left out: 2CBH cannot hold them yet. */
  chess960?: string;
}
/** Tablebase annotations of a game's main line (workbench). */
export interface TablebaseAnnotation {
  revision: string;
  items: {
    node: number;
    nag?: number;
    comment?: string;
    line: string[];
    lineComment?: string;
  }[];
  probed: number;
  entered?: number;
  mistakes: number;
  status: string;
}
/** A database symbol on the start screen (see docs/start-screen.md). */
export interface DatabaseSymbol {
  path: string;
  name: string;
  folder: string;
  /** One of ChessBase's 20 database types. */
  kind: string;
  favorite: boolean;
  exists: boolean;
  format?: "classic" | "new";
  entries?: number;
  problem?: string;
}
export interface FolderShortcut {
  path: string;
  name: string;
  exists: boolean;
}
export interface StartScreenData {
  favorites: DatabaseSymbol[];
  recent: DatabaseSymbol[];
  folders: FolderShortcut[];
  kinds: string[];
}
export interface FolderListing {
  symbols: DatabaseSymbol[];
  truncated: boolean;
}
