import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  DocumentResponse,
  PgnCollectionResponse,
  SaveResponse,
  Edit,
  OpenDatabaseResponse,
  PageResponse,
  GameResponse,
  PositionResponse,
} from "./types";
export const backend = {
  watchClose: async (handler: () => void) => {
    if (!isTauri()) return () => {};
    const off = await getCurrentWindow().onCloseRequested((event) => {
      event.preventDefault();
      handler();
    });
    const offQuit = await listen("workbench-exit-request", handler);
    return () => {
      off();
      offQuit();
    };
  },
  finishClose: () => invoke<void>("finish_close"),
  choosePgn: () =>
    open({ multiple: false, filters: [{ name: "PGN", extensions: ["pgn"] }] }),
  chooseSave: () =>
    save({
      defaultPath: "game.pgn",
      filters: [{ name: "PGN", extensions: ["pgn"] }],
    }),
  openPgn: (path: string, requestId: string) =>
    invoke<PgnCollectionResponse>("open_pgn", { path, requestId }),
  loadPgnGame: (collectionId: string, index: number, requestId: string) =>
    invoke<DocumentResponse>("load_pgn_game", {
      collectionId,
      index,
      requestId,
    }),
  closePgn: (collectionId: string) =>
    invoke<void>("close_pgn", { collectionId }),
  editDocument: (documentId: string, edit: Edit, expectedRevision: string) =>
    invoke<DocumentResponse>("edit_document", {
      documentId,
      edit,
      expectedRevision,
    }),
  saveDocument: (
    documentId: string,
    destination: string,
    expectedRevision: string,
    allowIncomplete: boolean,
  ) =>
    invoke<SaveResponse>("save_document", {
      documentId,
      destination,
      expectedRevision,
      allowIncomplete,
    }),
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
