import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { invoke, isTauri, Channel } from "@tauri-apps/api/core";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import type {
  EngineResponse,
  EngineEventDto,
  DocumentResponse,
  PgnCollectionResponse,
  SaveResponse,
  Edit,
  OpenDatabaseResponse,
  PageResponse,
  GameResponse,
  PositionResponse,
  StartupRequest,
  SaveGameResponse,
  GameFilter,
  SearchResponse,
} from "./types";
export const backend = {
  pickEngine: () =>
    open({ multiple: false, title: "Choose a local UCI engine executable" }),
  chooseEngine: (path: string) =>
    invoke<EngineResponse>("choose_engine", { path }),
  setEngineOption: (engineId: string, name: string, value: string) =>
    invoke<void>("set_engine_option", { engineId, name, value }),
  startAnalysis: async (
    engineId: string,
    documentId: string,
    nodeId: number,
    requestId: string,
    onEvent: (event: EngineEventDto) => void,
  ) => {
    const events = new Channel<EngineEventDto>();
    events.onmessage = onEvent;
    try {
      await invoke<void>("start_analysis", {
        engineId,
        documentId,
        nodeId,
        requestId,
        events,
      });
    } catch (e) {
      events.onmessage = () => {};
      throw e;
    }
    return () => {
      events.onmessage = () => {};
    };
  },
  stopAnalysis: (engineId: string) =>
    invoke<void>("stop_analysis", { engineId }),
  closeEngine: (engineId: string) => invoke<void>("close_engine", { engineId }),
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
      filters: [{ name: "Chess databases", extensions: ["cbh", "2cbh"] }],
    }),
  startupRequest: () => invoke<StartupRequest | null>("startup_request"),
  startupShown: () => invoke<void>("startup_shown"),
  openDatabase: (path: string, requestId: string, writable = false) =>
    invoke<OpenDatabaseResponse>("open_database", {
      path,
      requestId,
      writable,
    }),
  chooseNewDatabase: () =>
    save({
      title: "New database",
      defaultPath: "New Database.2cbh",
      filters: [{ name: "ChessBase database", extensions: ["2cbh"] }],
    }),
  confirmEditing: () =>
    ask(
      "Games you save, replace or delete will change this database's files. ChessBase's own index files (.2lcd, .2lgd) are not updated; ChessBase's Check Integrity rebuilds them. Keep a backup of databases you care about.",
      { title: "Open database for editing", kind: "warning" },
    ),
  createDatabase: (path: string, requestId: string) =>
    invoke<OpenDatabaseResponse>("create_database", { path, requestId }),
  saveGame: (
    documentId: string,
    databaseId: string,
    expectedRevision: string,
    allowIncomplete: boolean,
  ) =>
    invoke<SaveGameResponse>("save_game", {
      documentId,
      databaseId,
      expectedRevision,
      allowIncomplete,
    }),
  replaceGame: (
    documentId: string,
    expectedRevision: string,
    allowIncomplete: boolean,
  ) =>
    invoke<SaveGameResponse>("replace_game", {
      documentId,
      expectedRevision,
      allowIncomplete,
    }),
  setDeleted: (databaseId: string, ordinal: string, deleted: boolean) =>
    invoke<void>("set_deleted", { databaseId, ordinal, deleted }),
  searchGames: (databaseId: string, filter: GameFilter, requestId: string) =>
    invoke<SearchResponse>("search_games", { databaseId, filter, requestId }),
  cancelSearch: (databaseId: string) =>
    invoke<void>("cancel_search", { databaseId }),
  clearSearch: (databaseId: string) =>
    invoke<void>("clear_search", { databaseId }),
  readSearchPage: (
    databaseId: string,
    start: string,
    limit: number,
    requestId: string,
  ) =>
    invoke<PageResponse>("read_search_page", {
      databaseId,
      start,
      limit,
      requestId,
    }),
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
