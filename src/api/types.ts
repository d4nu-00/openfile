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
  | { kind: "undo" }
  | { kind: "redo" };
/** Edits that change the tree's shape; the selection is re-read after them. */
export const structuralEdits: Edit["kind"][] = [
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
}
