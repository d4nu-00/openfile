import { useEffect, useMemo, useRef, useState } from "react";
import type {
  GameView,
  Line,
  PageResponse,
  ReferenceResponse,
  ReferenceFilter,
  ReferenceStatistics,
  TreeResponse,
} from "../api/types";
import type { WorkbenchApi } from "../api/client";
import type { WorkbenchState } from "./workbench";
import { SelectionGate } from "./selection";

const explain = (e: unknown) =>
  (e as { message?: string })?.message ?? String(e);

/** The moves from the game's start to `node`. */
export function lineTo(game: GameView, node: number): Line | undefined {
  const moves: string[] = [];
  let id: number | undefined = node;
  let steps = 0;
  while (id !== undefined && id !== 0) {
    const n: GameView["nodes"][number] | undefined = game.nodes[id];
    if (!n?.uci || steps++ > game.nodes.length) return undefined;
    moves.push(n.uci);
    id = n.parent;
  }
  return id === 0
    ? { startFen: game.startFen, moves: moves.reverse() }
    : undefined;
}

export interface BookState {
  loading: boolean;
  tree?: TreeResponse;
  error?: string;
}
export interface ReferenceState {
  loading: boolean;
  response?: ReferenceResponse;
  page?: PageResponse;
  error?: string;
  statistics?: ReferenceStatistics;
  statisticsLoading?: boolean;
  statisticsError?: string;
}

export const defaultReferenceFilter: ReferenceFilter = {
  goodGames: false,
  playerBlack: false,
};

/** Waits this long after a move before asking, so fast replay stays smooth. */
const SETTLE_MS = 60;

/**
 * Keeps the book and the reference search in step with the board. Each new
 * position supersedes the previous request; a stale reference search is
 * cancelled in the backend.
 */
export function usePositionPanes(
  api: WorkbenchApi,
  state: WorkbenchState,
  active: { book: boolean; reference: boolean },
  /**
   * When set, the Book shows this database's preparation instead of its
   * tree; a new key (another player or colour) asks again.
   */
  prepared?: string,
  bookScope: "database" | "search" = "database",
) {
  const db = state.database;
  const game = state.document?.game;
  const node = state.position?.nodeId;
  const line = useMemo(
    () => (game && node !== undefined ? lineTo(game, node) : undefined),
    [game, node],
  );
  const key = line ? `${line.startFen}|${line.moves.join(" ")}` : "";
  const gates = useRef({
    book: new SelectionGate(),
    reference: new SelectionGate(),
    statistics: new SelectionGate(),
  }).current;
  const [book, setBook] = useState<BookState>({ loading: false });
  const [reference, setReference] = useState<ReferenceState>({
    loading: false,
  });
  const [referenceFilter, setReferenceFilter] = useState<ReferenceFilter>(
    defaultReferenceFilter,
  );
  useEffect(() => setReferenceFilter(defaultReferenceFilter), [db?.databaseId]);
  const filterKey = JSON.stringify(referenceFilter);
  /** The position whose Find Position the user asked for. */
  const [searchKey, setSearchKey] = useState<string>();
  const searchAnyway = key !== "" && searchKey === key;

  useEffect(() => {
    if (!active.book || !db || !line) return;
    const request = gates.book.begin();
    setBook({ loading: true });
    const filtered = bookScope === "search" && !!state.search && !prepared;
    const timer = setTimeout(async () => {
      try {
        const tree = prepared
          ? await api.preparedTree(db.databaseId, line, request)
          : filtered
            ? await api.filteredOpeningTree(db.databaseId, line, request)
            : await api.openingTree(db.databaseId, line, request);
        if (gates.book.accept(tree.requestId))
          setBook({ loading: false, tree });
      } catch (e) {
        if (gates.book.accept(request))
          setBook({ loading: false, error: explain(e) });
      }
    }, SETTLE_MS);
    return () => {
      clearTimeout(timer);
      gates.book.begin();
      if (filtered)
        void api
          .cancelFilteredOpeningTree(db.databaseId, request)
          .catch(() => {});
    };
    // The database object changes when its tree is built or deleted.
  }, [active.book, db, key, prepared, bookScope, state.search]);

  useEffect(() => {
    if (!active.reference || !db || !line) return;
    const request = gates.reference.begin();
    setReference({ loading: true });
    const timer = setTimeout(async () => {
      try {
        const response = await api.referenceSearch(
          db.databaseId,
          line,
          request,
          searchAnyway,
        );
        if (!gates.reference.accept(response.requestId)) return;
        if (response.cancelled) return;
        const page =
          response.hits === "0"
            ? { requestId: request, entries: [] }
            : await api.readReferencePage(db.databaseId, "1", 200, request);
        if (gates.reference.accept(page.requestId))
          setReference({ loading: false, response, page });
      } catch (e) {
        if (gates.reference.accept(request))
          setReference({ loading: false, error: explain(e) });
      }
    }, SETTLE_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [active.reference, db, key, searchAnyway]);

  useEffect(() => {
    const response = reference.response;
    if (
      !active.reference ||
      !db ||
      !line ||
      !response ||
      response.source === "notSearched"
    )
      return;
    const request = gates.statistics.begin();
    setReference((r) => ({
      ...r,
      statistics: undefined,
      statisticsLoading: true,
      statisticsError: undefined,
    }));
    const timer = setTimeout(async () => {
      try {
        const statistics = await api.referenceStatistics(
          db.databaseId,
          line,
          request,
          referenceFilter,
        );
        if (gates.statistics.accept(statistics.requestId))
          setReference((r) => ({ ...r, statistics, statisticsLoading: false }));
      } catch (e) {
        if (gates.statistics.accept(request))
          setReference((r) => ({
            ...r,
            statisticsLoading: false,
            statisticsError: explain(e),
          }));
      }
    }, SETTLE_MS);
    return () => {
      clearTimeout(timer);
      gates.statistics.begin();
      void api.cancelReference(db.databaseId).catch(() => {});
    };
  }, [
    active.reference,
    db?.databaseId,
    key,
    reference.response?.requestId,
    filterKey,
  ]);

  // Leaving the position or closing the pane stops a running search.
  useEffect(() => {
    if (!active.reference || !db) return;
    return () => {
      gates.reference.begin();
      void api.cancelReference(db.databaseId).catch(() => {});
    };
  }, [active.reference, db?.databaseId]);

  return {
    line,
    book,
    reference,
    referenceFilter,
    setReferenceFilter,
    /** Runs Find Position for the current position, however long it takes. */
    searchReference: () => setSearchKey(key),
    cancelReference: () => {
      gates.reference.begin();
      gates.statistics.begin();
      setSearchKey(undefined);
      setReference((r) => ({ ...r, loading: false, statisticsLoading: false }));
      if (db) void api.cancelReference(db.databaseId).catch(() => {});
    },
  };
}
