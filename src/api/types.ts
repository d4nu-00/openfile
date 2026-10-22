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
  /** the reference app's other medals (Best game, Novelty, Endgame, …). */
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
  /** Critical position mark: 1 opening (a repertoire move), 2 middlegame,
   * 3 endgame. */
  critical?: number;
  /** A training question about the position before the move. */
  training?: Training;
  /** Each move answer in SAN in that position (null when not legal). */
  trainingMoves?: (string | null)[];
  /** Medals, move time, diagram, pawn structure, piece path, stored
   * evaluation (docs/annotations.md). */
  extras?: MoveExtras;
}
export interface MoveExtras {
  /** Medal bits, as the game's medal word (MEDAL_NAMES). */
  medal: number;
  /** "h:mm:ss". */
  moveTime?: string;
  diagram: boolean;
  pawnStructure: boolean;
  /** The square whose piece's path is shown. */
  piecePath?: string;
  /** From White's side, "+0.35/22". */
  evaluation?: string;
}
export type ExtraEdit =
  | { kind: "medal"; bits: number }
  | { kind: "moveTime"; time: number | null }
  | { kind: "diagram"; on: boolean }
  | { kind: "pawnStructure"; on: boolean }
  | { kind: "piecePath"; on: boolean };
/** Training annotations (docs/training.md). */
export interface TrainingText {
  /** 0..6: en, de, fr, es, it, nl, pt. */
  language: number;
  index?: number;
  text: string;
}
export interface TrainingBlock {
  texts: TrainingText[];
  squares?: SquareMark[];
  arrows?: ArrowMark[];
  /** Strings the workbench keeps without showing. */
  other?: { slot: number; index: number; bytes: number[] }[];
}
export interface TrainingAnswer {
  from?: number;
  to?: number;
  extra?: number;
  points: number;
  choice?: TrainingBlock;
  feedback?: TrainingBlock;
}
export interface Training {
  kind: "moveInput" | "multipleChoice";
  seconds: number;
  score: number;
  question: TrainingBlock;
  wrong?: TrainingBlock;
  help?: [TrainingBlock, TrainingBlock];
  answers: TrainingAnswer[];
}
export type ReplayProgressMap = Record<
  string,
  { attempts: number; needsRetry: boolean; bestPoints?: number }
>;
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
  /** The evaluation profile stored with the game (docs/evaluation-profile.md). */
  profile?: ProfilePoint[];
}
/** One evaluation of a profile, from White's side. */
export interface ProfilePoint {
  /** Plies from the start; 0 is the start position. */
  ply: number;
  cp?: number;
  /** Moves to mate, positive when White mates. */
  mate?: number;
  /** Instant analysis's pass, or a stored depth. */
  depth: number;
}
/** Centipawn Analysis of the selected games of a list (docs/centipawn-analysis.md). */
export interface CentipawnPlayerGame {
  opponent: string;
  opponentElo: number;
  halfPoints?: number | null;
  value: number;
  /** × 100 */
  weight: number;
}
export interface CentipawnPlayerRow {
  name: string;
  elo: number;
  perform?: number | null;
  points: number;
  games: CentipawnPlayerGame[];
  weightedAverage?: number | null;
}
export interface CentipawnListReport {
  cancelled: boolean;
  error?: string | null;
  games: number;
  analysed: number;
  replaced: number;
  skippedCount: number;
  skipped: { ordinal: string; reason: string }[];
  millisPerMove: number;
  players: CentipawnPlayerRow[];
  csv: string;
}
export interface CentipawnListProgress {
  game: number;
  games: number;
  done: number;
  total: number;
  label: string;
}
/** Instant Analysis's state while it runs and at the end. */
export interface InstantProgress {
  revision: string;
  pass: number;
  millisPerPosition: number;
  done: number;
  total: number;
  profile: ProfilePoint[];
  finished: boolean;
  cancelled: boolean;
}
export interface InstantParams {
  engine: string;
  options: Record<string, string>;
  level: number;
  endless: boolean;
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
  | { kind: "setExtra"; node: NodeId; extra: ExtraEdit }
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
  | {
      kind: "setTraining";
      node: NodeId;
      training: Training | null;
      /** Per answer, its move in SAN or UCI; null keeps the stored one. */
      moves?: (string | null)[];
    }
  | { kind: "clearTraining" }
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
  /** Milliseconds the engine has searched (UCI `time`). */
  time?: number;
  diagnostics: string[];
}
/**
 * How a kibitzer searches: Analyse Threat (the other side to move) and the
 * moves Next Best / Search Better leave out (UCI).
 */
export interface SearchMode {
  threat: boolean;
  exclude: string[];
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
export type SearchScope = "all" | "within" | "add";
export interface MaterialRange {
  min?: number;
  max?: number;
}
export interface MaterialSide {
  queens?: MaterialRange;
  rooks?: MaterialRange;
  bishops?: MaterialRange;
  knights?: MaterialRange;
  pawns?: MaterialRange;
  minors?: MaterialRange;
  total?: MaterialRange;
}
export interface MaterialQuery {
  white?: MaterialSide;
  black?: MaterialSide;
  balance?: MaterialRange;
  ignoreColors?: boolean;
  first?: number;
  last?: number;
  length?: number;
  lines?: boolean;
}
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
  /** The Medals tab: games with any of these medal bits. */
  medals?: number;
  /** The Annotations tab; every criterion set must hold. */
  annotations?: AnnotationQuery;
  material?: MaterialQuery;
}
/** The search mask's Annotations tab (dialog 343; see docs/search.md). */
export interface AnnotationQuery {
  text1: string;
  text2: string;
  wholeWord: boolean;
  /** PGN NAG numbers; each must be in the game. */
  symbols: number[];
  anyText: boolean;
  anySymbols: boolean;
  colours: boolean;
  training: boolean;
  multimedia: boolean;
  pawnStructure: boolean;
  piecePath: boolean;
  criticalOpening: boolean;
  criticalMiddlegame: boolean;
  criticalEndgame: boolean;
  variations: boolean;
}
export const emptyAnnotations = (): AnnotationQuery => ({
  text1: "",
  text2: "",
  wholeWord: false,
  symbols: [],
  anyText: false,
  anySymbols: false,
  colours: false,
  training: false,
  multimedia: false,
  pawnStructure: false,
  piecePath: false,
  criticalOpening: false,
  criticalMiddlegame: false,
  criticalEndgame: false,
  variations: false,
});
/** The medals by bit (see docs/formats.md). */
export const MEDAL_NAMES = [
  "Best game",
  "Decided tournament",
  "Model game",
  "Novelty",
  "Pawn structure",
  "Strategy",
  "Tactics",
  "With attack",
  "Defense",
  "Sacrifice",
  "Material",
  "Piece play",
  "Endgame",
  "Tactical blunder",
  "Strategical blunder",
  "User",
];
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
/** Error Report (see docs/error-report.md). */
export interface ErrorReportParams {
  engine: string;
  options: Record<string, string>;
  /** the reference app's "Slower (top games)": depth 12 instead of 9. */
  slower: boolean;
  /** Count this player's moves; both sides when null. */
  player: number | null;
  ranges: { start: string; end: string }[];
}
export interface ContextRow {
  moves: number;
  errors: number;
  averageLoss?: number | null;
}
export interface ErrorReport {
  cancelled: boolean;
  depth: number;
  games: number;
  skipped: number;
  truncated: boolean;
  player?: string | null;
  moves: number;
  averageLoss?: number | null;
  errorValue?: number | null;
  pliesPerGame?: number | null;
  blunderElo?: number | null;
  rating?: number | null;
  difference?: number | null;
  severity: [number, number, number, number];
  distribution: number[];
  phases: Record<string, ContextRow>;
  opponents: Record<string, ContextRow>;
  transitions: number[][];
  worst: {
    ordinal: string;
    node: number;
    number: string;
    san: string;
    best?: string | null;
    loss: number;
  }[];
}
export interface ErrorProgress {
  gamesDone: number;
  games: number;
  positions: number;
}
export interface SelectionPlayer {
  id: number;
  name?: string | null;
  games: number;
}
/** Tournament report: the crosstable's games by round. */
export interface RoundGame {
  ordinal: number;
  /** Rows of the crosstable. */
  white: number;
  black: number;
  result: string;
  moves: number;
}
export interface ReportRound {
  /** 0 for games without a round. */
  round: number;
  date?: string;
  games: RoundGame[];
  standings: { row: number; points: number; games: number }[];
}
export interface TournamentReport {
  table: Crosstable;
  rounds: ReportRound[];
  stats: {
    games: number;
    whiteWins: number;
    draws: number;
    blackWins: number;
    forfeits: number;
    whiteScore?: number;
    averageMoves?: number;
  };
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
/** Player dossier. */
export interface ColourStats {
  games: number;
  whiteWins: number;
  draws: number;
  blackWins: number;
  score?: number;
  /** 0 (worst) … 3 (as expected) … 6 (best). */
  scoreBand?: number;
  opponentsAverage?: number;
  performance?: number;
  whiteWinMoves?: number;
  whiteWinBand?: number;
  drawMoves?: number;
  drawBand?: number;
  blackWinMoves?: number;
  blackWinBand?: number;
  drawPercent?: number;
  /** 0 (lowest) … 3 (average) … 5 (highest). */
  drawQuoteBand?: number;
  quickDrawPercent?: number;
}
export interface RatingBand {
  from: number;
  games: number;
  points: number;
  percent: number;
}
export interface DossierGame {
  ordinal: string;
  white: boolean;
  opponent?: string;
  year?: number;
  result?: string;
  moves: number;
  eco?: string;
  tournament?: string;
  medals: string[];
}
export interface Dossier {
  playerId: number;
  name?: string;
  games: number;
  deleted: number;
  truncated: boolean;
  firstYear: number;
  lastYear: number;
  years: YearStat[];
  all: ColourStats;
  white: ColourStats;
  black: ColourStats;
  ratingBands: RatingBand[];
  tournaments: TournamentStat[];
  opponents: OpponentStat[];
  moreTournaments: number;
  moreOpponents: number;
  repertoireWhite: RepertoireRow[];
  repertoireBlack: RepertoireRow[];
  medalGames: DossierGame[];
  medalCount: number;
  matesGiven: DossierGame[];
  matesGivenCount: number;
  matesReceived: DossierGame[];
  matesReceivedCount: number;
  matesRead: number;
  annotated: DossierGame[];
  annotatedCount: number;
}
export interface DossierProgress {
  stage: "games" | "lines" | "mates";
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
export type TreeStatus =
  "tree" | "line" | "notInTree" | "tooMany" | "noTree" | "filtered";
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
  /** Number of search hits scanned, present for a filtered tree. */
  sampledHits?: number;
  /** Number of hits in the current search result. */
  totalHits?: number;
}
/** Local repertoire overlay, independent of the open database. */
export type WorkspaceSide = "white" | "black";
export type WorkspaceAction = "add" | "remove" | "prefer" | "note";
export interface RepertoireChoice {
  uci: string;
  san: string;
  preferred: boolean;
  note: string;
  valid: boolean;
}
export interface RepertoirePracticeStatus {
  attempts: number;
  correct: number;
  needsRetry: boolean;
}
export interface RepertoirePosition {
  key: string;
  fen: string;
  line: Line;
  route: string;
  ownTurn: boolean;
  moves: RepertoireChoice[];
  practice: RepertoirePracticeStatus;
}
export interface RepertoireSnapshot {
  entries: RepertoirePosition[];
  current?: RepertoirePosition;
  legal: RepertoireChoice[];
  ownTurn: boolean;
  answeredReplies?: Record<string, boolean>;
  replyPositions?: Record<string, string>;
}
export interface RepertoireGrade {
  correct: boolean;
  playedSan: string;
  preferredSan?: string;
  practice: RepertoirePracticeStatus;
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
  /** the reference app's evaluation symbols where the evaluation changes. */
  evaluations: { node: number; nag: number }[];
  revision: string;
}
/** One line of an analysis result, from the analysed position. */
export interface LineItem {
  moves: string[];
  /** the reference app's evaluation symbol for the last move. */
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
/** the reference app's job types: Analysis (1) and Deep Analysis (2). */
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
  /** the reference app's "Maximum Board Windows": 1 to 8, 8 by default. */
  maxGameTabs: number;
  engines: InstalledEngine[];
  defaultEngine?: string;
  /** The folder of Syzygy tablebase files. */
  tablebaseDir?: string;
  /** Instant Analysis: on/off, 1–6 (× 35 ms per position), Endless. */
  instantAnalysis?: InstantSettings;
  /** The Evaluation Profile pane, and the profile above the notation. */
  evalProfilePane?: boolean;
  evalProfileNotation?: boolean;
}
export interface InstantSettings {
  enabled: boolean;
  level: number;
  endless: boolean;
}
export const defaultInstant: InstantSettings = {
  enabled: false,
  level: 3,
  endless: false,
};
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
export interface ReferenceFilter {
  eloMin?: number;
  eloMax?: number;
  yearMin?: number;
  yearMax?: number;
  goodGames: boolean;
  playerId?: number;
  /** Shared display name for the tab and extra pane; Rust ignores it. */
  playerName?: string;
  playerBlack: boolean;
}
export interface ReferenceStatistics {
  requestId: string;
  fullGames: number;
  filteredGames: number;
  playerGames: number;
  scannedGames: number;
  afterElo: number;
  afterYear: number;
  missingElo: number;
  missingYear: number;
  complete: boolean;
  fullComplete: boolean;
  moves: { san: string; full: number; filtered: number; player: number }[];
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
/** the reference app's Kill Doubles options that the workbench ports. */
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
  /** One of the reference app's 20 database types. */
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

/** A database's files that macOS removed to iCloud to save space. */
export interface CloudStatus {
  /** File names still in iCloud only. */
  files: string[];
  /** Their size in bytes, as a decimal string. */
  bytes: string;
}
export interface CloudProgress {
  filesDone: number;
  files: number;
  bytesDone: string;
  bytes: string;
}

/** Centipawn Analysis (docs/centipawn-analysis.md). */
export interface CentipawnParams {
  engine: string;
  options: Record<string, string>;
  /** The reference app's "Seconds", 0 to 60. */
  seconds: number;
  everyMove: boolean;
}
export interface CentipawnSide {
  moves: number;
  averageLoss?: number | null;
  /** Mean of 100 × exp(−centipawn loss / 150) for analysed moves. */
  accuracy?: number | null;
  errorValue?: number | null;
  /** Per category, in the reference app's order. */
  categories: number[];
}
export interface ReviewedMove {
  node: NodeId;
  ply: number;
  moveNumber: number;
  san: string;
  white: boolean;
  loss: number;
  category?:
    | "brilliant"
    | "strong"
    | "best"
    | "good"
    | "inaccuracy"
    | "mistake"
    | "missedWin"
    | "losing"
    | null;
  /** Engine continuation in SAN from the position before this move. */
  bestLine: string[];
  /** First engine move for checking a board attempt; never shown as notation. */
  bestUci?: string | null;
}
export interface CentipawnReport {
  revision: string;
  cancelled: boolean;
  positions: number;
  bookPlies: number;
  millisPerMove: number;
  white: CentipawnSide;
  black: CentipawnSide;
  /** Texts: node 0 is before the first move. */
  items: { node: NodeId; comment: string }[];
  moves: ReviewedMove[];
}
/** Generate Repertoire and the Repertoire Report (docs/repertoire.md). */
export interface RepertoireProgress {
  stage: "games" | "lines" | "writing" | "repertoire" | "scan";
  done: string;
  total: string;
}
export interface FrequentPlayer {
  id: number;
  name?: string | null;
  games: number;
}
export interface RepertoireSide {
  white: boolean;
  /** The database written; null when the side has no repertoire game. */
  path?: string | null;
  games: number;
  read: number;
  truncated: boolean;
  repertoireGames: number;
  merged: number;
  unclassified: number;
  rare: number;
  cut: number;
}
export interface GeneratedRepertoire {
  players: string[];
  sides: RepertoireSide[];
}
export interface HitGame {
  ordinal: string;
  white?: string | null;
  black?: string | null;
  whiteElo?: number | null;
  blackElo?: number | null;
  year?: number | null;
  result?: string | null;
}
export interface PositionHits {
  text: string;
  fen: string;
  games: number;
  ordinals: string[];
  top: HitGame[];
}
export interface RepertoireTopic {
  name: string;
  eco?: string | null;
  positions: PositionHits[];
}
export interface RepertoireReport {
  repertoire: string;
  repertoireGames: number;
  repertoireTruncated: boolean;
  positions: number;
  scanned: string;
  scanTruncated: boolean;
  matched: string;
  topics: RepertoireTopic[];
  positionsTruncated: boolean;
  cancelled: boolean;
}

/** Recovered native STYLE_REPORT dimensions with explicit missing dependencies. */
export interface StyleParams {
  gameTypes: number;
  playerIds: number[];
  white?: boolean | null;
  yearFrom?: number | null;
  yearTo?: number | null;
  minOpponentElo?: number | null;
  sampleLimit: number;
}
export interface StyleProgress {
  stage: string;
  done: string;
  total: string;
}
export interface StyleEvidence {
  ordinal: string;
  node: number;
  ply: number;
  san: string;
}
export interface NativeStyleInput {
  label: string;
  numerator: number | null;
  denominator: number | null;
  value: number | null;
  reference: number | null;
}
export interface NativeStyleDimension {
  id: string;
  label: string;
  description: string;
  rawScore: number | null;
  normalized: number | null;
  normalization: [number, number];
  grade: string | null;
  gradeReason: string;
  status: "available" | "partial" | "insufficient" | "unavailable";
  unavailableReason: string | null;
  inputs: NativeStyleInput[];
  evidence: StyleEvidence[];
}
export interface StyleGame {
  storedMoves?: number;
  ordinal: string;
  white: boolean;
  opponent?: string | null;
  date?: string | null;
  result?: string | null;
  plies: number;
  cut?: boolean;
}
export interface StyleReport {
  jobId: string;
  players: { id: number; name?: string | null }[];
  criteria: StyleParams;
  database: string;
  databaseEntries: string;
  eligible: number;
  selected: number;
  analysed: number;
  whiteGames: number;
  blackGames: number;
  deleted: number;
  filtered: number;
  skipped: number;
  cutGames: number;
  unknownDate?: number;
  unknownOpponentElo?: number;
  unknownResult?: number;
  skippedSetup?: number;
  skippedUnreadable?: number;
  skippedUnsupported?: number;
  skippedPartial?: number;
  firstDate?: string | null;
  lastDate?: string | null;
  tooShort: number;
  unknownCategory: number;
  dimensions: NativeStyleDimension[];
  games: StyleGame[];
  elapsedMs: number;
}
