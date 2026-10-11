import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  OpenDatabaseResponse,
  PageResponse,
  GameResponse,
  PositionResponse,
} from "./types";
export const backend = {
  chooseDatabase: () =>
    open({
      multiple: false,
      filters: [{ name: "Chess databases", extensions: ["cbh"] }],
    }),
  openDatabase: (path: string, requestId: string) =>
    invoke<OpenDatabaseResponse>("open_database", { path, requestId }),
  readPage: (
    databaseId: string,
    start: string,
    limit: number,
    requestId: string,
  ) =>
    invoke<PageResponse>("read_page", { databaseId, start, limit, requestId }),
  loadEntry: (databaseId: string, ordinal: string, requestId: string) =>
    invoke<GameResponse>("load_entry", { databaseId, ordinal, requestId }),
  positionAt: (documentId: string, nodeId: number, requestId: string) =>
    invoke<PositionResponse>("position_at", { documentId, nodeId, requestId }),
  closeDatabase: (databaseId: string) =>
    invoke<void>("close_database", { databaseId }),
  closeDocument: (documentId: string) =>
    invoke<void>("close_document", { documentId }),
};
export type WorkbenchApi = typeof backend;
