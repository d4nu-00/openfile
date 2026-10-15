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
  BoosterProgress,
  Line,
  TreeResponse,
  TreeProgress,
  ReferenceResponse,
  AppSettings,
  TablebaseResponse,
  BlunderParams,
  BlunderReport,
  DeepParams,
  DeepProgress,
  DeepReport,
  JobKind,
  JobsProgress,
  JobsView,
  RunJobsParams,
  RunJobsReport,
  SortColumn,
  SortProgress,
  SortResponse,
  NameKind,
  NamePage,
  Crosstable,
  CrossLayout,
  PasteResponse,
  ConvertEstimate,
  ConvertResponse,
  IntegrityReport,
  DoubleOptions,
  DoublesResponse,
  CompactResponse,
  SelectionEditResponse,
  SelectionNames,
  PlayerStatistics,
  PositionQuery,
  StatGroup,
  TablebaseAnnotation,
  StartScreenData,
  FolderListing,
} from "./types";

/** A progress channel; one that drops the messages when nobody listens. */
function channel<T>(onMessage?: (message: T) => void) {
  const events = new Channel<T>();
  if (onMessage) events.onmessage = onMessage;
  return events;
}
export const backend = {
  pickEngine: () =>
    open({ multiple: false, title: "Choose a local UCI engine executable" }),
  /** An engine's UCI name and author; the engine is stopped again. */
  probeEngine: (path: string) =>
    invoke<Record<string, string>>("probe_engine", { path }),
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
  newDocument: (fen: string | null, requestId: string) =>
    invoke<DocumentResponse>("new_document", { fen, requestId }),
  pasteGame: (text: string, requestId: string) =>
    invoke<DocumentResponse>("paste_game", { text, requestId }),
  documentPgn: (documentId: string) =>
    invoke<string>("document_pgn", { documentId }),
  readClipboard: () => navigator.clipboard.readText(),
  writeClipboard: (text: string) => navigator.clipboard.writeText(text),
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
  chooseConvertTarget: (name: string) =>
    save({
      title: "Convert to 2CBH",
      defaultPath: `${name}.2cbh`,
      filters: [{ name: "ChessBase database", extensions: ["2cbh"] }],
    }),
  /** The space a conversion needs and the free space at `target`. */
  convertEstimate: (
    databaseId: string,
    ranges: { start: string; end: string }[] | null,
    target: string,
  ) =>
    invoke<ConvertEstimate>("convert_estimate", {
      databaseId,
      ranges,
      target,
    }),
  convertDatabase: async (
    databaseId: string,
    ranges: { start: string; end: string }[] | null,
    target: string,
    onProgress: (p: { done: string; total: string }) => void,
  ) => {
    const events = new Channel<{ done: string; total: string }>();
    events.onmessage = onProgress;
    return invoke<ConvertResponse>("convert_database", {
      databaseId,
      ranges,
      target,
      events,
    });
  },
  checkIntegrity: async (
    databaseId: string,
    onProgress: (p: { done: string; total: string }) => void,
  ) => {
    const events = new Channel<{ done: string; total: string }>();
    events.onmessage = onProgress;
    return invoke<IntegrityReport>("check_integrity", { databaseId, events });
  },
  findDoubleGames: async (
    databaseId: string,
    options: DoubleOptions,
    onProgress: (p: { done: string; total: string }) => void,
  ) => {
    const events = new Channel<{ done: string; total: string }>();
    events.onmessage = onProgress;
    return invoke<DoublesResponse>("find_double_games", {
      databaseId,
      options,
      events,
    });
  },
  removeDeletedGames: async (
    databaseId: string,
    onProgress: (p: { done: string; total: string }) => void,
  ) => {
    const events = new Channel<{ done: string; total: string }>();
    events.onmessage = onProgress;
    return invoke<CompactResponse>("remove_deleted_games", {
      databaseId,
      events,
    });
  },
  /** Mark Games: toggles the user medal of the rows' games. */
  markRows: async (
    databaseId: string,
    ranges: { start: string; end: string }[],
    onProgress: (p: { done: string; total: string }) => void,
  ) => {
    const events = new Channel<{ done: string; total: string }>();
    events.onmessage = onProgress;
    return invoke<SelectionEditResponse>("mark_rows", {
      databaseId,
      ranges,
      events,
    });
  },
  /** Edit Game Data for a selection: tournament, annotator or source. */
  setNamesOfRows: async (
    databaseId: string,
    ranges: { start: string; end: string }[],
    names: SelectionNames,
    onProgress: (p: { done: string; total: string }) => void,
  ) => {
    const events = new Channel<{ done: string; total: string }>();
    events.onmessage = onProgress;
    return invoke<SelectionEditResponse>("set_names_of_rows", {
      databaseId,
      ranges,
      names,
      events,
    });
  },
  cancelMaintenance: (databaseId: string) =>
    invoke<void>("cancel_maintenance", { databaseId }),
  confirmRemoveDeleted: () =>
    ask(
      "Games marked as deleted are removed from the database files for good, and the other games are numbered anew. The files are rewritten beside the database and swapped in when complete. ChessBase's index files (.2lcd, .2lgd) are removed; its Check Integrity rebuilds them. Files beside the database that refer to game numbers, such as ChessBase's key files, are not updated.",
      { title: "Remove Deleted Games", kind: "warning", okLabel: "Remove" },
    ),
  cancelConvert: (databaseId: string) =>
    invoke<void>("cancel_convert", { databaseId }),
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
  searchGames: (
    databaseId: string,
    filter: GameFilter,
    requestId: string,
    onProgress?: (p: BoosterProgress) => void,
  ) =>
    invoke<SearchResponse>("search_games", {
      databaseId,
      filter,
      requestId,
      events: channel(onProgress),
    }),
  findPosition: (
    databaseId: string,
    fen: string,
    requestId: string,
    onProgress?: (p: BoosterProgress) => void,
  ) =>
    invoke<SearchResponse>("find_position", {
      databaseId,
      fen,
      requestId,
      events: channel(onProgress),
    }),
  /** The search mask's position tab: a pattern or an exact position. */
  searchPosition: (
    databaseId: string,
    query: PositionQuery,
    requestId: string,
    onProgress?: (p: BoosterProgress) => void,
  ) =>
    invoke<SearchResponse>("search_position", {
      databaseId,
      query,
      requestId,
      events: channel(onProgress),
    }),
  createSearchBooster: async (
    databaseId: string,
    onProgress: (p: BoosterProgress) => void,
  ) => {
    const events = new Channel<BoosterProgress>();
    events.onmessage = onProgress;
    return invoke<boolean>("create_search_booster", { databaseId, events });
  },
  deleteSearchBooster: (databaseId: string) =>
    invoke<void>("delete_search_booster", { databaseId }),
  createOpeningTree: async (
    databaseId: string,
    onProgress: (p: TreeProgress) => void,
  ) => {
    const events = new Channel<TreeProgress>();
    events.onmessage = onProgress;
    return invoke<boolean>("create_opening_tree", { databaseId, events });
  },
  cancelOpeningTree: (databaseId: string) =>
    invoke<void>("cancel_opening_tree", { databaseId }),
  deleteOpeningTree: (databaseId: string) =>
    invoke<void>("delete_opening_tree", { databaseId }),
  openingTree: (databaseId: string, line: Line, requestId: string) =>
    invoke<TreeResponse>("opening_tree", { databaseId, line, requestId }),
  referenceSearch: (
    databaseId: string,
    line: Line,
    requestId: string,
    searchAnyway?: boolean,
  ) =>
    invoke<ReferenceResponse>("reference_search", {
      databaseId,
      line,
      requestId,
      searchAnyway: searchAnyway ?? false,
    }),
  cancelReference: (databaseId: string) =>
    invoke<void>("cancel_reference", { databaseId }),
  readReferencePage: (
    databaseId: string,
    start: string,
    limit: number,
    requestId: string,
  ) =>
    invoke<PageResponse>("read_reference_page", {
      databaseId,
      start,
      limit,
      requestId,
    }),
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
  /** Rows of the game list: the database or its search, sorted or not. */
  readRows: (
    databaseId: string,
    start: string,
    limit: number,
    requestId: string,
  ) =>
    invoke<PageResponse>("read_rows", { databaseId, start, limit, requestId }),
  findRow: (databaseId: string, ordinal: string) =>
    invoke<string | null>("find_row", { databaseId, ordinal }),
  sortList: async (
    databaseId: string,
    column: SortColumn | null,
    descending: boolean,
    onProgress: (p: SortProgress) => void,
  ) => {
    const events = new Channel<SortProgress>();
    events.onmessage = onProgress;
    return invoke<SortResponse>("sort_list", {
      databaseId,
      column,
      descending,
      events,
    });
  },
  cancelSort: (databaseId: string) =>
    invoke<void>("cancel_sort", { databaseId }),
  /** Prepares the player or tournament list; rows is absent if cancelled. */
  nameList: async (
    databaseId: string,
    kind: NameKind,
    onProgress: (p: { done: string; total: string }) => void,
  ) => {
    const events = new Channel<{ done: string; total: string }>();
    events.onmessage = onProgress;
    return invoke<{ rows?: string }>("name_list", { databaseId, kind, events });
  },
  cancelNameList: (databaseId: string) =>
    invoke<void>("cancel_name_list", { databaseId }),
  readNames: (
    databaseId: string,
    kind: NameKind,
    start: string,
    limit: number,
    requestId: string,
  ) =>
    invoke<NamePage>("read_names", {
      databaseId,
      kind,
      start,
      limit,
      requestId,
    }),
  findName: (databaseId: string, kind: NameKind, prefix: string) =>
    invoke<string>("find_name", { databaseId, kind, prefix }),
  /** Copy: remembers the games in the given rows of the current list. */
  copyRows: (databaseId: string, ranges: { start: string; end: string }[]) =>
    invoke<{ games: string; truncated: boolean }>("copy_rows", {
      databaseId,
      ranges,
    }),
  pasteGames: async (
    databaseId: string,
    onProgress: (p: { done: string; total: string }) => void,
  ) => {
    const events = new Channel<{ done: string; total: string }>();
    events.onmessage = onProgress;
    return invoke<PasteResponse>("paste_games", { databaseId, events });
  },
  cancelPaste: (databaseId: string) =>
    invoke<void>("cancel_paste", { databaseId }),
  /** Delete: marks the rows' games deleted, or undeletes them all. */
  deleteRows: (databaseId: string, ranges: { start: string; end: string }[]) =>
    invoke<string>("delete_rows", { databaseId, ranges }),
  confirmPaste: (games: string) =>
    ask(
      `Append ${games} games to this database? Annotations that are not decoded, or that ChessBase cannot store, are left out; the games affected are counted.`,
      { title: "Paste games", kind: "warning" },
    ),
  crosstable: (
    databaseId: string,
    tournamentId: number,
    layout: CrossLayout | null,
  ) => invoke<Crosstable>("crosstable", { databaseId, tournamentId, layout }),
  /** Player statistics; undefined when cancelled. */
  playerStatistics: async (
    databaseId: string,
    playerId: number,
    onProgress: (p: { done: string; total: string }) => void,
  ) => {
    const events = new Channel<{ done: string; total: string }>();
    events.onmessage = onProgress;
    return (
      (await invoke<PlayerStatistics | null>("player_statistics", {
        databaseId,
        playerId,
        events,
      })) ?? undefined
    );
  },
  cancelPlayerStatistics: (databaseId: string) =>
    invoke<void>("cancel_player_statistics", { databaseId }),
  /** Shows the games of a statistics row as the list's search result. */
  statisticsGames: (
    databaseId: string,
    playerId: number,
    group: StatGroup,
    requestId: string,
  ) =>
    invoke<SearchResponse>("statistics_games", {
      databaseId,
      playerId,
      group,
      requestId,
    }),
  loadEntry: (databaseId: string, ordinal: string, requestId: string) =>
    invoke<GameResponse>("load_entry", { databaseId, ordinal, requestId }),
  positionAt: (documentId: string, nodeId: number, requestId: string) =>
    invoke<PositionResponse>("position_at", { documentId, nodeId, requestId }),
  closeDatabase: (databaseId: string) =>
    invoke<void>("close_database", { databaseId }),
  closeDocument: (documentId: string) =>
    invoke<void>("close_document", { documentId }),
  getSettings: () => invoke<AppSettings>("get_settings"),
  startScreen: () => invoke<StartScreenData>("start_screen"),
  folderDatabases: (folder: string) =>
    invoke<FolderListing>("folder_databases", { folder }),
  pinDatabase: (path: string) => invoke<void>("pin_database", { path }),
  unpinDatabase: (path: string) => invoke<void>("unpin_database", { path }),
  setDatabaseKind: (path: string, kind: string) =>
    invoke<void>("set_database_kind", { path, kind }),
  forgetRecent: (path: string) => invoke<void>("forget_recent", { path }),
  addFolderShortcut: (folder: string) =>
    invoke<void>("add_folder_shortcut", { folder }),
  removeFolderShortcut: (folder: string) =>
    invoke<void>("remove_folder_shortcut", { folder }),
  chooseFolder: () => open({ directory: true, title: "Add a folder shortcut" }),
  chooseTablebaseFolder: () =>
    open({ directory: true, title: "Choose the folder of Syzygy tablebases" }),
  blunderCheck: (
    documentId: string,
    params: BlunderParams,
    onProgress: (p: { done: number; total: number }) => void,
  ) => {
    const events = new Channel<{ done: number; total: number }>();
    events.onmessage = onProgress;
    return invoke<BlunderReport>("blunder_check", {
      documentId,
      params,
      events,
    });
  },
  cancelBlunderCheck: (documentId: string) =>
    invoke<void>("cancel_blunder_check", { documentId }),
  /** Deep Analysis of one position; the tree comes back as lines. */
  deepAnalysis: (
    documentId: string,
    params: DeepParams,
    onProgress: (p: DeepProgress) => void,
  ) => {
    const events = new Channel<DeepProgress>();
    events.onmessage = onProgress;
    return invoke<DeepReport>("deep_analysis", { documentId, params, events });
  },
  /** Stops Deep Analysis, keeping what it found, or discards it. */
  stopDeepAnalysis: (documentId: string, discard: boolean) =>
    invoke<void>("stop_deep_analysis", { documentId, discard }),
  analysisJobs: () => invoke<JobsView>("analysis_jobs"),
  /** Create analysis jobs: the final positions of the selected games. */
  addAnalysisJobs: (
    databaseId: string,
    ranges: { start: string; end: string }[],
    kind: JobKind,
  ) =>
    invoke<{ view: JobsView; leftOut: number }>("add_analysis_jobs", {
      databaseId,
      ranges,
      kind,
    }),
  /** Add Analysis Job: the board position of the shown game. */
  addAnalysisJob: (documentId: string, node: number, kind: JobKind) =>
    invoke<JobsView>("add_analysis_job", { documentId, node, kind }),
  editAnalysisJob: (id: number, action: "restart" | "skip" | "remove") =>
    invoke<JobsView>("edit_analysis_job", { id: String(id), action }),
  clearAnalysisJobs: () => invoke<JobsView>("clear_analysis_jobs"),
  runAnalysisJobs: (
    params: RunJobsParams,
    onProgress: (p: JobsProgress) => void,
  ) => {
    const events = new Channel<JobsProgress>();
    events.onmessage = onProgress;
    return invoke<RunJobsReport>("run_analysis_jobs", { params, events });
  },
  stopAnalysisJobs: () => invoke<void>("stop_analysis_jobs"),
  /** The game's main line annotated from the tablebases. */
  tablebaseAnnotation: (documentId: string) =>
    invoke<TablebaseAnnotation>("tablebase_annotation", { documentId }),
  probeTablebase: (fen: string, requestId: string) =>
    invoke<TablebaseResponse>("probe_tablebase", { fen, requestId }),
  setSettings: (settings: AppSettings) =>
    invoke<AppSettings>("set_settings", { settings }),
};
export type WorkbenchApi = typeof backend;
