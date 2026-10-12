export type NodeId = number;
export type Status =
  "available" | "missing" | "unsupported" | "corrupt" | "notDecoded";
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
}
export interface GameView {
  tags: Record<string, string>;
  startFen: string;
  nodes: GameNode[];
}
export interface OpenDatabaseResponse {
  requestId: string;
  databaseId: string;
  entryCount: string;
  companions: { extension: string; status: Status }[];
  diagnostics: Diagnostic[];
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
  | { kind: "setNags"; node: NodeId; nags: number[] };
export interface DocumentResponse {
  documentId: string;
  requestId?: string;
  game: GameView;
  revision: string;
  dirty: boolean;
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
