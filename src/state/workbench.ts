import { useEffect, useRef, useState, type SetStateAction } from "react";
import type {
  SearchResponse,
  SearchScope,
  GameFilter,
  GameResponse,
  OpenDatabaseResponse,
  PositionResponse,
  Diagnostic,
  EntrySummary,
  PositionQuery,
  SortColumn,
  StatGroup,
  Line,
} from "../api/types";
import type { WorkbenchApi } from "../api/client";
import { SelectionGate } from "./selection";
import { RowCache } from "./gameList";
/** The database entry a document was loaded from or last saved to. */
export interface Origin {
  databaseId: string;
  ordinal: string;
}
/** The current search result; the list pages through its hits. */
export interface ActiveSearch {
  /** The search mask behind a game-data search. */
  filter?: GameFilter;
  /** The board behind a position search. */
  fen?: string;
  /** The position tab behind a position search. */
  position?: PositionQuery;
  /** What the list shows, e.g. "Games of Carlsen, Magnus". */
  label?: string;
  scope?: SearchScope;
  hits: string;
  truncated: boolean;
}
/** The column the list is sorted by. */
export interface ListSort {
  column: SortColumn;
  descending: boolean;
}
/** An open database other than the active one, as its list was left. */
export interface ParkedDatabase {
  database: OpenDatabaseResponse;
  search?: ActiveSearch;
  sort?: ListSort;
  listVersion?: number;
  top: number;
  cursor?: number;
}
/** A game: its document, selected move, unsaved flag and origin. */
export interface GameSlot {
  document?: GameResponse;
  position?: PositionResponse;
  dirty?: boolean;
  origin?: Origin;
}
/** A game tab. The shown tab's fields live in `WorkbenchState` instead. */
export interface GameTab extends GameSlot {
  id: string;
  /** When the tab was last shown, for choosing a tab to reuse. */
  used: number;
}
export interface WorkbenchState {
  /** The active database: its list is shown. */
  database?: OpenDatabaseResponse;
  /** Other open databases, in tab order. */
  parked?: ParkedDatabase[];
  origin?: Origin;
  search?: ActiveSearch;
  sort?: ListSort;
  /** Changes whenever the list's rows change: search, sort or a save. */
  listVersion?: number;
  /** The game shown: the focused tab's, or the preview's. */
  document?: GameResponse;
  position?: PositionResponse;
  dirty?: boolean;
  /** Game tabs, in tab order. */
  tabs?: GameTab[];
  /** The tab whose game is shown; none while the preview is shown. */
  focus?: string;
  /** The preview's game while a tab is shown. */
  preview?: GameSlot;
}
const slotOf = (s: WorkbenchState): GameSlot => ({
  document: s.document,
  position: s.position,
  dirty: s.dirty,
  origin: s.origin,
});
let used = 0;
/** Shows another game: tab `target`, or the preview. The shown fields go
 * back to their owner. */
export function refocus(
  s: WorkbenchState,
  target: string | undefined,
): WorkbenchState {
  if (s.focus === target) return s;
  if (target !== undefined && !s.tabs?.some((t) => t.id === target)) return s;
  let tabs = s.tabs ?? [];
  let preview = s.preview;
  if (s.focus === undefined) preview = slotOf(s);
  else tabs = tabs.map((t) => (t.id === s.focus ? { ...t, ...slotOf(s) } : t));
  const next =
    target === undefined ? preview : tabs.find((t) => t.id === target);
  if (target === undefined) preview = undefined;
  else tabs = tabs.map((t) => (t.id === target ? { ...t, used: ++used } : t));
  return {
    ...s,
    tabs,
    preview,
    focus: target,
    document: next?.document,
    position: next?.position,
    dirty: next?.dirty,
    origin: next?.origin,
  };
}
/** Each tab with its current game, including the shown tab's. */
export function tabGames(s: WorkbenchState): GameTab[] {
  return (s.tabs ?? []).map((t) =>
    t.id === s.focus ? { ...t, ...slotOf(s) } : t,
  );
}
/** The games of every tab and the preview, shown or not. */
export function allGames(s: WorkbenchState): GameSlot[] {
  return [
    ...tabGames(s),
    s.focus === undefined ? slotOf(s) : (s.preview ?? {}),
  ];
}
/** Opens a new tab holding `slot` and shows it. */
export function addTab(s: WorkbenchState, slot: GameSlot): WorkbenchState {
  const id = `tab-${++used}`;
  const parked = refocus(s, undefined);
  // Showing the preview took it out of `preview`; keep it there.
  const kept = { ...parked, preview: slotOf(parked) };
  return refocus(
    { ...kept, tabs: [...(kept.tabs ?? []), { id, used, ...slot }] },
    id,
  );
}
/** Moves the preview's game into a new tab and shows it. */
export function promotePreview(s: WorkbenchState): WorkbenchState {
  if (s.focus !== undefined || !s.document) return s;
  return addTab(
    {
      ...s,
      document: undefined,
      position: undefined,
      dirty: false,
      origin: undefined,
    },
    slotOf(s),
  );
}
/** Removes tab `id`; when it is shown, the last used tab, or else the
 * preview, is shown instead. */
export function removeTab(s: WorkbenchState, id: string): WorkbenchState {
  const rest = (s.tabs ?? []).filter((t) => t.id !== id);
  if (s.focus !== id) return { ...s, tabs: rest };
  const next = [...rest].sort((a, b) => b.used - a.used)[0];
  const cleared = {
    ...s,
    tabs: rest,
    focus: undefined,
    ...(s.preview ?? {
      document: undefined,
      position: undefined,
      dirty: false,
      origin: undefined,
    }),
    preview: undefined,
  };
  return next ? refocus(cleared, next.id) : cleared;
}
/** Column names for messages. */
export const sortLabels: Record<SortColumn, string> = {
  white: "White",
  whiteElo: "Elo W",
  black: "Black",
  blackElo: "Elo B",
  result: "Result",
  eco: "ECO",
  tournament: "Tournament",
  round: "Round",
  date: "Date",
  annotator: "Annotator",
  source: "Source",
  moves: "Moves",
};
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
  /** A search running on the active database: its database and progress. */
  const [searching, setSearching] = useState<{
    databaseId: string;
    percent?: number;
  }>();
  const [loadingEntry, setLoadingEntry] = useState<string>();
  /** The first row shown, and the row of the game last chosen from the list. */
  const [listTop, setListTopState] = useState(0);
  const listTopRef = useRef(0);
  const setListTop = (top: SetStateAction<number>) => {
    const next = typeof top === "function" ? top(listTopRef.current) : top;
    listTopRef.current = next;
    setListTopState(next);
  };
  const [cursor, setCursorState] = useState<number>();
  const cursorRef = useRef<number>(undefined);
  const setCursor = (row: number | undefined) => {
    cursorRef.current = row;
    setCursorState(row);
  };
  /** Progress of a running sort. */
  const [sorting, setSorting] = useState<string>();
  const [, rowsArrived] = useState(0);
  const rows = useRef(
    new RowCache<EntrySummary>(
      (...args) => api.readRows(...args).then((page) => page.entries),
      () => rowsArrived((n) => n + 1),
      (message) => setDiagnostics([{ code: "rows", message }]),
    ),
  ).current;
  rows.reset(
    `${state.database?.databaseId}:${state.listVersion ?? 0}`,
    state.database?.databaseId ?? "",
  );
  /** Rows in the list: the database, or its search result. */
  const countRows = (s: WorkbenchState) =>
    s.database ? Number(s.search ? s.search.hits : s.database.entryCount) : 0;
  const rowCount = countRows(state);
  /** Progress of a running opening tree build. */
  const [treeBuild, setTreeBuild] = useState<string>();
  /** The request of a running iCloud download, for Stop. */
  const [cloudDownload, setCloudDownload] = useState<string>();
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
      for (const g of allGames(current.current))
        closeDoc(g.document?.documentId);
      closeDb(current.current.database?.databaseId);
      for (const p of current.current.parked ?? [])
        closeDb(p.database.databaseId);
    },
    [api],
  );
  /** Opens `chosen`, or asks for a database when none is given. */
  function openDatabase(chosen?: string) {
    return openWith(async (request) => {
      const path = chosen ?? (await api.chooseDatabase());
      if (typeof path !== "string" || !gates.database.accept(request)) return;
      if (!(await fromCloud(path, request))) return;
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
      if (!(await fromCloud(path, request))) return;
      setStatus("Opening database…");
      // A database is never open for reading and writing at once; the
      // backend refuses while the same files are open in another tab.
      return api.openDatabase(path, request, true);
    });
  }
  /** Opens a .2cbh database the workbench just wrote, for editing. */
  function openWritten(path: string) {
    return openWith(async (request) => {
      setStatus("Opening database…");
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
  /**
   * Downloads the database's files that macOS keeps only in iCloud, after
   * asking: opening it would otherwise wait for them without progress.
   * False when the user declines or another open supersedes this one;
   * Stop rejects.
   */
  async function fromCloud(path: string, request: string) {
    const cloud = await api.cloudStatus(path);
    if (cloud.files.length === 0) return true;
    const name = path
      .split(/[\\/]/)
      .pop()!
      .replace(/\.[^.]*$/, "");
    const ok = await api.confirmCloudDownload(
      name,
      cloud.files.length,
      size(cloud.bytes),
    );
    if (!ok || !gates.database.accept(request)) return false;
    setCloudDownload(request);
    setStatus(`Downloading “${name}” from iCloud…`);
    try {
      await api.downloadCloudFiles(path, request, (p) => {
        if (gates.database.accept(request))
          setStatus(
            `Downloading “${name}” from iCloud: ${size(p.bytesDone)} of ${size(p.bytes)}, ${p.filesDone} of ${p.files} files`,
          );
      });
    } finally {
      setCloudDownload(undefined);
    }
    return gates.database.accept(request);
  }
  function cancelCloudDownload() {
    if (cloudDownload)
      void api.cancelCloudDownload(cloudDownload).catch(() => {});
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
      gates.page.begin();
      // The previous database stays open in its tab; the game stays loaded.
      setState((s) => ({
        ...s,
        parked: [...(s.parked ?? []), ...park(s)],
        database,
        search: undefined,
        sort: undefined,
        listVersion: (s.listVersion ?? 0) + 1,
      }));
      setListTop(0);
      setCursor(undefined);
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
  /** The active database's list, to keep while another is shown. */
  function park(s: WorkbenchState): ParkedDatabase[] {
    if (!s.database) return [];
    return [
      {
        database: s.database,
        search: s.search,
        sort: s.sort,
        listVersion: s.listVersion,
        top: listTopRef.current,
        cursor: cursorRef.current,
      },
    ];
  }
  /** Shows the list of another open database. */
  function switchDatabase(databaseId: string) {
    const s = current.current;
    const target = s.parked?.find((p) => p.database.databaseId === databaseId);
    if (!target) return;
    gates.page.begin();
    setState((s) => ({
      ...s,
      parked: [
        ...(s.parked ?? []).filter((p) => p.database.databaseId !== databaseId),
        ...park(s),
      ],
      database: target.database,
      search: target.search,
      sort: target.sort,
      listVersion: (s.listVersion ?? 0) + 1,
    }));
    setListTop(target.top);
    setCursor(target.cursor);
    setStatus(
      `${target.database.entryCount} entries${target.database.writable ? " · editable" : ""}`,
    );
  }
  /** Closes an open database; the next open one becomes active. */
  function closeDatabase(databaseId: string) {
    const s = current.current;
    closeDb(databaseId);
    const origin = s.origin?.databaseId === databaseId ? undefined : s.origin;
    // Games from the database stay open, but no longer belong to it.
    const unlink = <T extends GameSlot>(g: T): T =>
      g.origin?.databaseId === databaseId ? { ...g, origin: undefined } : g;
    setState((s) => ({
      ...s,
      tabs: s.tabs?.map(unlink),
      preview: s.preview && unlink(s.preview),
    }));
    if (s.database?.databaseId !== databaseId) {
      setState((s) => ({
        ...s,
        origin,
        parked: (s.parked ?? []).filter(
          (p) => p.database.databaseId !== databaseId,
        ),
      }));
      return;
    }
    const next = s.parked?.at(-1);
    gates.page.begin();
    setState((s) => ({
      ...s,
      origin,
      parked: (s.parked ?? []).slice(0, -1),
      database: next?.database,
      search: next?.search,
      sort: next?.sort,
      listVersion: (s.listVersion ?? 0) + 1,
    }));
    setListTop(next?.top ?? 0);
    setCursor(next?.cursor);
    setStatus(
      next
        ? `${next.database.entryCount} entries`
        : "Open a CBH database to begin.",
    );
  }
  /** Reads the list's rows again, e.g. after a save. A sorted list is sorted
   * again, since saving drops its sort order. */
  async function refreshList() {
    const db = current.current.database;
    if (!db) return;
    const sort = current.current.sort;
    if (sort) {
      await sortBy(sort.column, sort.descending, true);
      return;
    }
    setState((s) => ({ ...s, listVersion: (s.listVersion ?? 0) + 1 }));
  }
  /** Sorts the list by `column`, or returns to entry order. */
  async function sortBy(column?: SortColumn, descending = false, keep = false) {
    const db = current.current.database;
    if (!db) return;
    const label = column ? sortLabels[column] : "";
    const show = (text?: string) => {
      setSorting(text);
      if (text) setStatus(`Sorting by ${label}: ${text}`);
    };
    show(column ? "starting…" : undefined);
    try {
      const sorted = await api.sortList(
        db.databaseId,
        column ?? null,
        descending,
        (p) => {
          const pct = Number((BigInt(p.done) * 100n) / BigInt(p.total || "1"));
          show(
            p.stage === "names"
              ? "reading names…"
              : p.stage === "reading"
                ? `reading entries, ${pct}%`
                : `arranging, ${pct}%`,
          );
        },
      );
      if (current.current.database?.databaseId !== db.databaseId) return;
      if (sorted.cancelled) {
        setStatus("Sorting cancelled");
        return;
      }
      setState((s) => ({
        ...s,
        sort: sorted.column
          ? { column: sorted.column, descending: sorted.descending }
          : undefined,
        listVersion: (s.listVersion ?? 0) + 1,
      }));
      if (!keep) {
        setListTop(0);
        setCursor(undefined);
      }
      setStatus(
        sorted.column
          ? `Sorted by ${sortLabels[sorted.column]}${sorted.descending ? ", descending" : ""}`
          : "Entry order",
      );
    } catch (e) {
      setDiagnostics([{ code: "sort", message: explain(e) }]);
      setStatus("Sorting failed");
    } finally {
      setSorting(undefined);
    }
  }
  function cancelSort() {
    const db = current.current.database;
    if (db) void api.cancelSort(db.databaseId).catch(() => {});
  }
  /** The row (from 0) of an entry in the list, if the list has it. */
  async function rowOf(databaseId: string, ordinal: string) {
    const loaded = rows.find((r) => r.ordinal === ordinal);
    if (loaded !== undefined) return loaded;
    const found = await api.findRow(databaseId, ordinal).catch(() => null);
    return found === null || found === undefined
      ? undefined
      : Number(found) - 1;
  }
  /** Scrolls to the row of entry `ordinal` and loads it. */
  async function goToEntry(ordinal: string) {
    const db = current.current.database;
    if (!db) return;
    const row = await rowOf(db.databaseId, ordinal);
    if (current.current.database?.databaseId !== db.databaseId) return;
    if (row !== undefined) {
      setCursor(row);
      setListTop(Math.max(0, row - 3));
    }
    const entry =
      row === undefined
        ? undefined
        : await rows.entry(row, countRows(current.current));
    await loadEntry(ordinal, entry?.foundNode ?? 0);
  }
  /** Loads the game `delta` rows away from the current one in the list. */
  async function stepGame(delta: 1 | -1) {
    const db = current.current.database;
    const ordinal = current.current.document?.summary.ordinal;
    if (!db || !ordinal || current.current.origin?.databaseId !== db.databaseId)
      return;
    const row =
      cursorRef.current !== undefined &&
      rows.row(cursorRef.current)?.ordinal === ordinal
        ? cursorRef.current
        : await rowOf(db.databaseId, ordinal);
    if (row === undefined) return;
    const next = row + delta;
    const entry = await rows.entry(next, countRows(current.current));
    if (!entry || current.current.database?.databaseId !== db.databaseId)
      return;
    setCursor(next);
    await loadEntry(entry.ordinal, entry.foundNode ?? 0);
  }
  /** Previews the game at `row` of the list. */
  function previewRow(row: number, entry: EntrySummary) {
    setCursor(row);
    return loadEntry(entry.ordinal, entry.foundNode ?? 0);
  }
  /** Searches the whole database and shows the hits in the list. */
  function search(
    filter: GameFilter,
    label?: string,
    scope: SearchScope = "all",
  ) {
    return runSearch({ filter, label, scope }, (db, request, progress) =>
      scope === "all"
        ? api.searchGames(db, filter, request, progress)
        : api.searchGames(db, filter, request, progress, scope),
    );
  }
  /** Lists the games of a player statistics row. */
  function statisticsGames(playerId: number, group: StatGroup, label: string) {
    return runSearch({ label }, (db, request) =>
      api.statisticsGames(db, playerId, group, request),
    );
  }
  /** Lists the games of a preparation, or those reaching `line`. */
  function preparationGames(
    playerId: number,
    white: boolean,
    line: Line | null,
    label: string,
  ) {
    return runSearch({ label }, (db, request) =>
      api.preparationGames(db, playerId, white, line, request),
    );
  }
  /** Lists the given entries. */
  function showEntries(ordinals: string[], label: string) {
    return runSearch({ label }, (db, request) =>
      api.showEntries(db, ordinals, request),
    );
  }
  /** Finds games that reach the position `fen`. */
  function findPosition(fen: string) {
    return runSearch({ fen }, (db, request, progress) =>
      api.findPosition(db, fen, request, progress),
    );
  }
  /** Runs the search mask's position tab. */
  function positionSearch(position: PositionQuery, scope: SearchScope = "all") {
    return runSearch({ position, scope }, (db, request, progress) =>
      scope === "all"
        ? api.searchPosition(db, position, request, progress)
        : api.searchPosition(db, position, request, progress, scope),
    );
  }
  async function runSearch(
    what: {
      filter?: GameFilter;
      scope?: SearchScope;
      fen?: string;
      position?: PositionQuery;
      label?: string;
    },
    run: (
      databaseId: string,
      request: string,
      progress: (p: { done: string; total: string }) => void,
    ) => Promise<SearchResponse>,
  ) {
    const db = current.current.database;
    if (!db) return;
    const request = gates.page.begin();
    const byPosition = !!(what.fen || what.position);
    const verb = byPosition ? "Searching position" : "Searching";
    setStatus(`${verb}…`);
    setSearching({ databaseId: db.databaseId });
    const progress = (p: { done: string; total: string }) => {
      if (!gates.page.accept(request)) return;
      const total = BigInt(p.total);
      const percent =
        total > 0n ? Number((BigInt(p.done) * 100n) / total) : 100;
      setSearching({ databaseId: db.databaseId, percent });
      setStatus(`${verb}… ${percent}%`);
    };
    try {
      const found = await run(db.databaseId, request, progress);
      if (
        !gates.page.accept(found.requestId) ||
        current.current.database?.databaseId !== db.databaseId
      )
        return;
      if (found.cancelled) {
        setStatus("Search cancelled");
        return;
      }
      setState((s) => ({
        ...s,
        search: { ...what, hits: found.hits, truncated: found.truncated },
        listVersion: (s.listVersion ?? 0) + 1,
      }));
      setListTop(0);
      setCursor(undefined);
      const exact = what.fen || what.position?.exact;
      setStatus(
        `${found.hits} games found${found.truncated ? " (stopped at the limit)" : ""}${
          exact && !found.indexed ? " · no search booster" : ""
        }`,
      );
    } catch (e) {
      if (gates.page.accept(request)) {
        setDiagnostics([{ code: "search", message: explain(e) }]);
        setStatus("Search failed");
      }
    } finally {
      if (gates.page.accept(request)) setSearching(undefined);
    }
  }
  /** Builds the search booster for fast position search. */
  async function createBooster() {
    const db = current.current.database;
    if (!db) return;
    setStatus("Search booster: starting…");
    try {
      const built = await api.createSearchBooster(db.databaseId, (p) => {
        const pct = Number((BigInt(p.done) * 100n) / BigInt(p.total || "1"));
        setStatus(`Search booster: ${pct}% of ${p.total} entries`);
      });
      if (current.current.database?.databaseId !== db.databaseId) return;
      setState((s) => ({
        ...s,
        database: s.database && { ...s.database, booster: built },
      }));
      setStatus(
        built ? "Search booster is complete" : "Search booster cancelled",
      );
    } catch (e) {
      setDiagnostics([{ code: "booster", message: explain(e) }]);
      setStatus("Search booster failed");
    }
  }
  /** Builds the opening tree used by the book and the reference search. */
  async function createTree() {
    const db = current.current.database;
    if (!db) return;
    const show = (text: string) => {
      setTreeBuild(text);
      setStatus(`Opening tree: ${text}`);
    };
    show("starting…");
    try {
      const built = await api.createOpeningTree(db.databaseId, (p) => {
        const pct = Number((BigInt(p.done) * 100n) / BigInt(p.total || "1"));
        show(
          p.stage === "reading"
            ? `reading games, ${pct}% of ${p.total}`
            : `sorting positions, pass ${p.done} of ${p.total}`,
        );
      });
      if (current.current.database?.databaseId !== db.databaseId) return;
      setState((s) => ({
        ...s,
        database: s.database && { ...s.database, tree: built },
      }));
      setStatus(built ? "Opening tree is complete" : "Opening tree cancelled");
    } catch (e) {
      setDiagnostics([{ code: "tree", message: explain(e) }]);
      setStatus("Opening tree failed");
    } finally {
      setTreeBuild(undefined);
    }
  }
  async function deleteTree() {
    const db = current.current.database;
    if (!db) return;
    try {
      await api.deleteOpeningTree(db.databaseId);
      setState((s) => ({
        ...s,
        database: s.database && { ...s.database, tree: false },
      }));
      setStatus("Opening tree deleted");
    } catch (e) {
      setDiagnostics([{ code: "tree", message: explain(e) }]);
    }
  }
  function cancelTree() {
    const db = current.current.database;
    if (db) void api.cancelOpeningTree(db.databaseId).catch(() => {});
  }
  async function deleteBooster() {
    const db = current.current.database;
    if (!db) return;
    try {
      await api.deleteSearchBooster(db.databaseId);
      setState((s) => ({
        ...s,
        database: s.database && { ...s.database, booster: false },
      }));
      setStatus("Search booster deleted");
    } catch (e) {
      setDiagnostics([{ code: "booster", message: explain(e) }]);
    }
  }
  function cancelSearch() {
    const db = current.current.database;
    if (db) void api.cancelSearch(db.databaseId).catch(() => {});
  }
  /** Shows the whole database again. */
  /**
   * After Remove Deleted Games: games from `databaseId` follow their new
   * entry numbers ("0" for a removed entry), and the list shows the whole
   * database again.
   */
  function renumber(
    databaseId: string,
    moved: [string, string][],
    entryCount: string,
  ) {
    const map = new Map(moved);
    const follow = <T extends GameSlot>(g: T): T => {
      const next =
        g.origin?.databaseId === databaseId
          ? map.get(g.origin.ordinal)
          : undefined;
      if (next === undefined) return g;
      return {
        ...g,
        origin: next === "0" ? undefined : { databaseId, ordinal: next },
      };
    };
    setState((s) => ({
      ...follow(s),
      tabs: s.tabs?.map(follow),
      preview: s.preview && follow(s.preview),
      search: s.database?.databaseId === databaseId ? undefined : s.search,
      database:
        s.database?.databaseId === databaseId
          ? { ...s.database, entryCount }
          : s.database,
      listVersion: (s.listVersion ?? 0) + 1,
    }));
    setListTop(0);
    setCursor(undefined);
  }
  async function clearSearch() {
    const db = current.current.database;
    if (!db || !current.current.search) return;
    void api.clearSearch(db.databaseId).catch(() => {});
    setState((s) => ({
      ...s,
      search: undefined,
      listVersion: (s.listVersion ?? 0) + 1,
    }));
    setListTop(0);
    setCursor(undefined);
    setStatus(`${db.entryCount} entries`);
  }
  /** The entry being loaded, so a second click on it does not load it again. */
  const loading = useRef<{
    databaseId: string;
    ordinal: string;
    done: Promise<void>;
  }>(undefined);
  /** Whether entry `ordinal` of the active database is the game shown, once
   * a load of it that is under way has finished. */
  async function showing(ordinal: string) {
    const db = current.current.database?.databaseId;
    const l = loading.current;
    if (l && l.databaseId === db && l.ordinal === ordinal) await l.done;
    const s = current.current;
    return (
      !!s.document &&
      s.database?.databaseId === db &&
      s.origin?.databaseId === db &&
      s.origin?.ordinal === ordinal
    );
  }
  /** Loads an entry and selects `node` (a position search's hit) or the
   * start. The game replaces the one shown, or goes to `place` (a new tab). */
  function loadEntry(
    ordinal: string,
    node = 0,
    place?: (slot: GameSlot) => Promise<boolean>,
    styleJobId?: string,
  ) {
    const db = current.current.database;
    if (!db) return Promise.resolve();
    const done = loadEntryNow(db, ordinal, node, place, styleJobId);
    loading.current = { databaseId: db.databaseId, ordinal, done };
    void done.finally(() => {
      if (loading.current?.done === done) loading.current = undefined;
    });
    return done;
  }
  async function loadEntryNow(
    db: OpenDatabaseResponse,
    ordinal: string,
    node: number,
    place?: (slot: GameSlot) => Promise<boolean>,
    styleJobId?: string,
  ) {
    const request = gates.game.begin();
    gates.position.begin();
    setLoadingEntry(ordinal);
    setStatus(`Loading entry ${ordinal}…`);
    let candidate: string | undefined;
    try {
      const document = styleJobId
        ? await api.loadStyleEntry(db.databaseId, styleJobId, ordinal, request)
        : await api.loadEntry(db.databaseId, ordinal, request);
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
      const position = await api
        .positionAt(document.documentId, node, posId)
        .catch(() => api.positionAt(document.documentId!, 0, posId));
      if (
        !gates.game.accept(request) ||
        !gates.position.accept(position.requestId)
      ) {
        closeDoc(candidate);
        return;
      }
      const slot = {
        document,
        position,
        dirty: false,
        origin: { databaseId: db.databaseId, ordinal },
      };
      if (place) {
        // The new tab closes the document itself when it is refused.
        candidate = undefined;
        if (!(await place(slot))) return;
      } else {
        if (current.current.document?.documentId !== document.documentId)
          closeDoc(current.current.document?.documentId);
        setState((s) => ({ ...s, ...slot }));
      }
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
  /** Shows tab `target`, or the preview. Requests under way for the game
   * shown before are dropped. */
  function focusGame(target: string | undefined) {
    if (current.current.focus === target) return;
    gates.game.begin();
    gates.position.begin();
    setLoadingEntry(undefined);
    setState((s) => refocus(s, target));
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
    loadingEntry,
    switchDatabase,
    closeDatabase,
    rows,
    rowCount,
    listTop,
    setListTop,
    cursor,
    sorting,
    sortBy,
    cancelSort,
    refreshList,
    goToEntry,
    stepGame,
    previewRow,
    openDatabase,
    search,
    statisticsGames,
    preparationGames,
    showEntries,
    findPosition,
    positionSearch,
    searching,
    createBooster,
    deleteBooster,
    treeBuild,
    cloudDownload,
    cancelCloudDownload,
    createTree,
    deleteTree,
    cancelTree,
    cancelSearch,
    clearSearch,
    openDatabaseForEditing,
    openWritten,
    renumber,
    createDatabase,
    loadEntry,
    showing,
    selectNode,
    focusGame,
  };
}

/** Bytes as a short size, e.g. "1.5 GB". */
function size(bytes: string) {
  const n = Number(bytes);
  const units = ["bytes", "KB", "MB", "GB", "TB"];
  let i = 0;
  let v = n;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  return i === 0 ? `${n} bytes` : `${v.toFixed(1)} ${units[i]}`;
}
