import { useEffect, useRef, useState, type SetStateAction } from "react";
import type {
  GameResponse,
  OpenDatabaseResponse,
  PageResponse,
  PositionResponse,
  Diagnostic,
} from "../api/types";
import type { WorkbenchApi } from "../api/client";
import { SelectionGate } from "./selection";
/** The database entry a document was loaded from or last saved to. */
export interface Origin {
  databaseId: string;
  ordinal: string;
}
export interface WorkbenchState {
  database?: OpenDatabaseResponse;
  origin?: Origin;
  page?: PageResponse;
  document?: GameResponse;
  position?: PositionResponse;
  dirty?: boolean;
}
const explain = (e: unknown) =>
  (e as { message?: string })?.message ?? String(e);
export function useWorkbench(api: WorkbenchApi, initial: WorkbenchState = {}) {
  const [state, renderState] = useState(initial);
  const current = useRef(state);
  const setState = (update: SetStateAction<WorkbenchState>) => {
    const next =
      typeof update === "function" ? update(current.current) : update;
    current.current = next;
    renderState(next);
  };
  const [diagnostics, setDiagnostics] = useState<Diagnostic[]>([]);
  const [status, setStatus] = useState("Open a CBH database to begin.");
  const [pageStart, setPageStart] = useState("1");
  const [loadingEntry, setLoadingEntry] = useState<string>();
  const gates = useRef({
    database: new SelectionGate(),
    page: new SelectionGate(),
    game: new SelectionGate(),
    position: new SelectionGate(),
  }).current;
  const closeDoc = (id?: string) => {
    if (id) void api.closeDocument(id).catch(() => {});
  };
  const closeDb = (id?: string) => {
    if (id) void api.closeDatabase(id).catch(() => {});
  };
  useEffect(
    () => () => {
      Object.values(gates).forEach((g) => g.begin());
      closeDoc(current.current.document?.documentId);
      closeDb(current.current.database?.databaseId);
    },
    [api],
  );
  /** Opens `chosen`, or asks for a database when none is given. */
  function openDatabase(chosen?: string) {
    return openWith(async (request) => {
      const path = chosen ?? (await api.chooseDatabase());
      if (typeof path !== "string" || !gates.database.accept(request)) return;
      setStatus("Opening database…");
      return api.openDatabase(path, request);
    });
  }
  /** Opens a .2cbh database so games can be saved into it, after a warning. */
  function openDatabaseForEditing() {
    return openWith(async (request) => {
      const path = await api.chooseDatabase();
      if (typeof path !== "string" || !gates.database.accept(request)) return;
      if (!/\.2cbh$/i.test(path)) {
        setDiagnostics([
          {
            code: "readOnly",
            message:
              "Only .2cbh databases can be edited; classic .cbh databases open read-only.",
          },
        ]);
        return;
      }
      if (!(await api.confirmEditing())) return;
      setStatus("Opening database…");
      // A database is never open for reading and writing at once, so the
      // current one is closed first.
      const open = current.current.database?.databaseId;
      if (open) await api.closeDatabase(open).catch(() => {});
      setState((s) => ({ ...s, database: undefined, page: undefined }));
      return api.openDatabase(path, request, true);
    });
  }
  /** Creates a new, empty .2cbh database and opens it for editing. */
  function createDatabase() {
    return openWith(async (request) => {
      const path = await api.chooseNewDatabase();
      if (typeof path !== "string" || !gates.database.accept(request)) return;
      setStatus("Creating database…");
      return api.createDatabase(path, request);
    });
  }
  async function openWith(
    opener: (request: string) => Promise<OpenDatabaseResponse | undefined>,
  ) {
    const request = gates.database.begin();
    let candidate: string | undefined;
    try {
      const database = await opener(request);
      if (!database) {
        setStatus(
          current.current.database
            ? `${current.current.database.entryCount} entries`
            : "Open a CBH database to begin.",
        );
        return;
      }
      candidate = database.databaseId;
      if (!gates.database.accept(database.requestId)) {
        closeDb(candidate);
        return;
      }
      const pageId = gates.page.begin();
      const page =
        database.entryCount === "0"
          ? { requestId: pageId, entries: [] }
          : await api.readPage(database.databaseId, "1", 200, pageId);
      if (
        !gates.database.accept(request) ||
        !gates.page.accept(page.requestId)
      ) {
        closeDb(candidate);
        return;
      }
      gates.game.begin();
      gates.position.begin();
      setLoadingEntry(undefined);
      closeDb(current.current.database?.databaseId);
      closeDoc(current.current.document?.documentId);
      setState((s) => ({
        ...s,
        database,
        page,
        document: undefined,
        position: undefined,
        origin: undefined,
        dirty: false,
      }));
      setPageStart("1");
      setDiagnostics(database.diagnostics);
      setStatus(
        `${database.entryCount} entries${database.writable ? " · editable" : ""}`,
      );
    } catch (e) {
      closeDb(candidate);
      if (gates.database.accept(request)) {
        setDiagnostics([{ code: "open", message: explain(e) }]);
        setStatus("Database open failed");
      }
    }
  }
  async function readPage(start: string) {
    const db = current.current.database;
    if (!db) return;
    const request = gates.page.begin();
    try {
      const page = await api.readPage(db.databaseId, start, 200, request);
      if (
        !gates.page.accept(page.requestId) ||
        current.current.database?.databaseId !== db.databaseId
      )
        return;
      setState((s) => ({ ...s, page }));
      setPageStart(start);
      setDiagnostics([]);
    } catch (e) {
      if (gates.page.accept(request))
        setDiagnostics([{ code: "page", message: explain(e) }]);
    }
  }
  async function loadEntry(ordinal: string) {
    const db = current.current.database;
    if (!db) return;
    const request = gates.game.begin();
    gates.position.begin();
    setLoadingEntry(ordinal);
    setStatus(`Loading entry ${ordinal}…`);
    let candidate: string | undefined;
    try {
      const document = await api.loadEntry(db.databaseId, ordinal, request);
      candidate = document.documentId;
      if (
        !gates.game.accept(document.requestId) ||
        current.current.database?.databaseId !== db.databaseId
      ) {
        closeDoc(candidate);
        return;
      }
      if (!document.documentId || !document.game) {
        setDiagnostics(
          document.diagnostics.length
            ? document.diagnostics
            : [
                {
                  code: document.movesStatus,
                  message: `Replay unavailable: ${document.movesStatus}`,
                },
              ],
        );
        setStatus("Replay unavailable");
        return;
      }
      const posId = gates.position.begin();
      const position = await api.positionAt(document.documentId, 0, posId);
      if (
        !gates.game.accept(request) ||
        !gates.position.accept(position.requestId)
      ) {
        closeDoc(candidate);
        return;
      }
      if (current.current.document?.documentId !== candidate)
        closeDoc(current.current.document?.documentId);
      setState((s) => ({
        ...s,
        document,
        position,
        dirty: false,
        origin: { databaseId: db.databaseId, ordinal },
      }));
      setDiagnostics([
        ...document.summary.diagnostics,
        ...document.diagnostics,
      ]);
      setStatus(`Entry ${ordinal} · ${document.game.nodes.length - 1} moves`);
    } catch (e) {
      closeDoc(candidate);
      if (gates.game.accept(request)) {
        setDiagnostics([{ code: "load", message: explain(e) }]);
        setStatus("Game load failed");
      }
    } finally {
      if (gates.game.accept(request)) setLoadingEntry(undefined);
    }
  }
  async function selectNode(nodeId: number) {
    const document = current.current.document;
    if (!document?.documentId) return;
    const request = gates.position.begin();
    try {
      const position = await api.positionAt(
        document.documentId,
        nodeId,
        request,
      );
      if (
        !gates.position.accept(position.requestId) ||
        current.current.document?.documentId !== document.documentId
      )
        return;
      setState((s) => ({ ...s, position }));
    } catch (e) {
      if (gates.position.accept(request))
        setDiagnostics([{ code: "position", message: explain(e) }]);
    }
  }
  return {
    state,
    gates,
    setState,
    current,
    diagnostics,
    setDiagnostics,
    status,
    setStatus,
    pageStart,
    loadingEntry,
    openDatabase,
    openDatabaseForEditing,
    createDatabase,
    readPage,
    loadEntry,
    selectNode,
  };
}
