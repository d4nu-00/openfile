import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { vi } from "vitest";
import { App } from "../App";
import { backend } from "../api/client";
import type {
  GameResponse,
  PositionResponse,
  EntrySummary,
} from "../api/types";
export const startFen =
  "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
export const summary: EntrySummary = {
  ordinal: "1",
  kind: "game",
  deleted: false,
  white: "Alpha",
  black: "Beta",
  event: "Synthetic",
  result: "*",
  diagnostics: [],
};
export const selected: GameResponse = {
  requestId: "initial",
  documentId: "doc-1",
  revision: "0",
  dirty: false,
  summary,
  metadataStatus: "available",
  movesStatus: "available",
  annotationsStatus: "notDecoded",
  diagnostics: [],
  game: {
    startFen,
    tags: { White: "Alpha", Black: "Beta", Result: "*" },
    nodes: [
      { id: 0, children: [1], comments: [], nags: [] },
      {
        id: 1,
        parent: 0,
        uci: "e2e4",
        san: "e4",
        children: [2, 3],
        comments: ["Centre"],
        nags: [1],
      },
      {
        id: 2,
        parent: 1,
        uci: "e7e5",
        san: "e5",
        children: [],
        comments: [],
        nags: [],
      },
      {
        id: 3,
        parent: 1,
        uci: "c7c5",
        san: "c5",
        children: [],
        comments: [],
        nags: [],
      },
    ],
  },
};
export const database = {
  requestId: "initial",
  databaseId: "db-1",
  entryCount: "401",
  companions: [],
  diagnostics: [],
};
export const page = {
  requestId: "initial",
  entries: [
    summary,
    { ...summary, ordinal: "2", deleted: true },
    { ...summary, ordinal: "3", kind: "text" as const },
  ],
  next: "201",
};
/** Rows of the mock list: the page's three entries, then plain games. */
export function mockRows(start: number, limit: number, count = 401) {
  const rows = [];
  for (let i = start; i < Math.min(count + 1, start + limit); i++)
    rows.push(page.entries[i - 1] ?? { ...summary, ordinal: String(i) });
  return rows;
}
export function mockApi() {
  let game = structuredClone(selected.game!);
  let revision = 0;
  // The list shows these entries while a search is active.
  let searching = false;
  const api = {
    ...backend,
    pickEngine: vi.fn().mockResolvedValue("/synthetic/mock-engine"),
    chooseEngine: vi.fn().mockResolvedValue({
      engineId: "engine-1",
      identity: { name: "Mock engine" },
      options: [
        {
          name: "Threads",
          kind: "spin",
          default: "1",
          min: 1,
          max: 64,
          vars: [],
        },
        {
          name: "Playing Style",
          kind: "combo",
          default: "Normal Play",
          vars: ["Normal Play", "Very Solid"],
        },
      ],
      status: "ready",
    }),
    setEngineOption: vi.fn().mockResolvedValue(undefined),
    startAnalysis: vi.fn().mockResolvedValue(undefined),
    stopAnalysis: vi.fn().mockResolvedValue(undefined),
    closeEngine: vi.fn().mockResolvedValue(undefined),
    watchClose: vi.fn().mockResolvedValue(() => {}),
    finishClose: vi.fn().mockResolvedValue(undefined),
    choosePgn: vi.fn().mockResolvedValue("/synthetic/two.pgn"),
    chooseSave: vi.fn().mockResolvedValue("/synthetic/save.pgn"),
    openPgn: vi.fn().mockImplementation(async (_path, requestId) => ({
      requestId,
      collectionId: "pgn-1",
      gameCount: 2,
      games: [
        { White: "First", Black: "Beta" },
        { White: "Second", Black: "Beta" },
      ],
    })),
    loadPgnGame: vi.fn().mockImplementation(async (_id, index, requestId) => ({
      documentId: `pgn-doc-${index}`,
      requestId,
      game: {
        ...game,
        tags: { White: index ? "Second" : "First", Black: "Beta" },
      },
      revision: "0",
      dirty: false,
    })),
    closePgn: vi.fn().mockResolvedValue(undefined),
    editDocument: vi
      .fn()
      .mockImplementation(async (documentId, edit, expectedRevision) => {
        if (expectedRevision !== String(revision))
          throw { message: "Stale revision" };
        if (edit.kind === "appendMove") {
          const existing = game.nodes[edit.parent].children.find(
            (id) => game.nodes[id].uci === edit.uci,
          );
          if (existing === undefined) {
            const id = game.nodes.length;
            game.nodes.push({
              id,
              parent: edit.parent,
              uci: edit.uci,
              san: edit.uci.slice(2, 4),
              children: [],
              comments: [],
              nags: [],
            });
            game.nodes[edit.parent].children.push(id);
            revision++;
          }
        } else if (edit.kind === "setComments" || edit.kind === "setNags") {
          game.nodes[edit.node][
            edit.kind === "setComments" ? "comments" : "nags"
          ] = edit.comments ?? edit.nags;
          revision++;
        } else if (edit.kind === "setPreComments") {
          game.nodes[edit.node].preComments = edit.comments;
          revision++;
        } else if (edit.kind === "setTags") {
          if (edit.tags.Result === "3-0") throw { message: "Unknown result" };
          game.tags = { ...edit.tags };
          revision++;
        } else {
          // Structural edits: the backend's tree logic is tested in Rust;
          // here the game stays and the start is selected.
          revision++;
          return {
            documentId,
            game: structuredClone(game),
            revision: String(revision),
            dirty: true,
            selected: 0,
          };
        }
        return {
          documentId,
          game: structuredClone(game),
          revision: String(revision),
          dirty: revision > 0,
        };
      }),
    newDocument: vi
      .fn()
      .mockImplementation(async (fen: string | null, requestId: string) => {
        if (fen?.startsWith("8/8/8/8/8/8/8/8"))
          throw { message: "Invalid position: no kings" };
        return {
          documentId: "doc-new",
          requestId,
          revision: "0",
          dirty: false,
          game: {
            startFen: fen ?? startFen,
            tags: { White: "?", Black: "?", Result: "*" },
            nodes: [{ id: 0, children: [], comments: [], nags: [] }],
          },
        };
      }),
    pasteGame: vi
      .fn()
      .mockImplementation(async (_text: string, requestId: string) => ({
        documentId: "doc-pasted",
        requestId,
        revision: "0",
        dirty: false,
        game: {
          startFen,
          tags: { White: "Pasted", Black: "Game", Result: "1-0" },
          nodes: [{ id: 0, children: [], comments: [], nags: [] }],
        },
      })),
    documentPgn: vi.fn().mockResolvedValue('[Result "*"]\n\n1. e4 *\n'),
    readClipboard: vi.fn().mockResolvedValue(""),
    writeClipboard: vi.fn().mockResolvedValue(undefined),
    saveDocument: vi
      .fn()
      .mockImplementation(async (_id, destination, expectedRevision) => ({
        savedRevision: expectedRevision,
        destination,
      })),
    chooseDatabase: vi.fn().mockResolvedValue("/synthetic/test.cbh"),
    startupRequest: vi.fn().mockResolvedValue(null),
    startupShown: vi.fn().mockResolvedValue(undefined),
    chooseNewDatabase: vi.fn().mockResolvedValue("/synthetic/new.2cbh"),
    confirmEditing: vi.fn().mockResolvedValue(true),
    createDatabase: vi.fn().mockImplementation(async (_path, requestId) => ({
      ...database,
      entryCount: "0",
      writable: true,
      requestId,
    })),
    saveGame: vi
      .fn()
      .mockImplementation(async (_doc, databaseId, expectedRevision) => ({
        databaseId,
        ordinal: "201",
        entryCount: "201",
        savedRevision: expectedRevision,
      })),
    replaceGame: vi.fn().mockImplementation(async (_doc, expectedRevision) => ({
      databaseId: "db-1",
      ordinal: "1",
      entryCount: "200",
      savedRevision: expectedRevision,
    })),
    setDeleted: vi.fn().mockResolvedValue(undefined),
    searchGames: vi.fn().mockImplementation(async (_db, _filter, requestId) => {
      searching = true;
      return {
        requestId,
        hits: "2",
        scanned: "401",
        truncated: false,
        cancelled: false,
      };
    }),
    cancelSearch: vi.fn().mockResolvedValue(undefined),
    findPosition: vi.fn().mockImplementation(async (_db, _fen, requestId) => {
      searching = true;
      return {
        requestId,
        hits: "2",
        scanned: "401",
        truncated: false,
        cancelled: false,
        indexed: true,
      };
    }),
    createSearchBooster: vi.fn().mockImplementation(async (_db, onProgress) => {
      onProgress({ done: "401", total: "401" });
      return true;
    }),
    deleteSearchBooster: vi.fn().mockResolvedValue(undefined),
    createOpeningTree: vi.fn().mockImplementation(async (_db, onProgress) => {
      onProgress({ stage: "reading", done: "401", total: "401" });
      onProgress({ stage: "sorting", done: "1", total: "1" });
      return true;
    }),
    cancelOpeningTree: vi.fn().mockResolvedValue(undefined),
    deleteOpeningTree: vi.fn().mockResolvedValue(undefined),
    openingTree: vi.fn().mockImplementation(async (_db, line, requestId) => ({
      requestId,
      status: "tree",
      games: 10,
      ended: 1,
      moves:
        line.moves.length === 0
          ? [
              {
                uci: "e2e4",
                san: "e4",
                games: 6,
                share: 60,
                score: 55,
                draws: 30,
                averageElo: 2500,
                performance: 2535,
                firstYear: 1990,
                lastYear: 2024,
              },
              {
                uci: "d2d4",
                san: "d4",
                games: 3,
                share: 30,
                score: 50,
                draws: 40,
                averageElo: null,
                performance: null,
                firstYear: null,
                lastYear: null,
              },
            ]
          : [],
    })),
    referenceSearch: vi
      .fn()
      .mockImplementation(async (_db, _line, requestId) => ({
        requestId,
        hits: "2",
        truncated: false,
        cancelled: false,
        source: "tree",
        indexed: false,
      })),
    cancelReference: vi.fn().mockResolvedValue(undefined),
    readReferencePage: vi
      .fn()
      .mockImplementation(async (_db, _start, _limit, requestId) => ({
        requestId,
        entries: [
          { ...summary, ordinal: "9", foundNode: 1 },
          { ...summary, ordinal: "12", foundNode: 3 },
        ],
      })),
    clearSearch: vi.fn().mockImplementation(async () => {
      searching = false;
    }),
    readSearchPage: vi
      .fn()
      .mockImplementation(async (_db, _start, _limit, requestId) => ({
        requestId,
        entries: [
          { ...summary, ordinal: "7" },
          { ...summary, ordinal: "42" },
        ],
      })),
    openDatabase: vi
      .fn()
      .mockImplementation(async (_path, requestId, writable = false) => ({
        ...database,
        requestId,
        writable,
      })),
    readPage: vi
      .fn()
      .mockImplementation(async (_db, start, limit, requestId) => ({
        requestId,
        entries: mockRows(Number(start), limit),
      })),
    readRows: vi.fn(),
    findRow: vi
      .fn()
      .mockImplementation(async (_db: string, ordinal: string) =>
        searching
          ? ordinal === "42"
            ? "2"
            : ordinal === "7"
              ? "1"
              : null
          : ordinal,
      ),
    sortList: vi
      .fn()
      .mockImplementation(
        async (_db: string, column: string | null, descending: boolean) => ({
          rows: searching ? "2" : "401",
          cancelled: false,
          column: column ?? undefined,
          descending,
        }),
      ),
    cancelSort: vi.fn().mockResolvedValue(undefined),
    loadEntry: vi.fn().mockImplementation(async (_db, ordinal, requestId) => ({
      ...selected,
      summary: { ...summary, ordinal },
      requestId,
    })),
    positionAt: vi
      .fn()
      .mockImplementation(
        async (documentId, nodeId, requestId): Promise<PositionResponse> => ({
          requestId,
          documentId,
          nodeId,
          fen:
            nodeId === 0
              ? startFen
              : "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
          turn: nodeId === 0 ? "white" : "black",
          legalUci: nodeId === 0 ? ["e2e4", "d2d4"] : ["e7e5", "c7c5"],
        }),
      ),
    closeDatabase: vi.fn().mockResolvedValue(undefined),
    closeDocument: vi.fn().mockResolvedValue(undefined),
  };
  // The list reads search pages while searching, as the backend does.
  api.readRows.mockImplementation(
    (db: string, start: string, limit: number, requestId: string) =>
      searching
        ? api.readSearchPage(db, start, limit, requestId)
        : api.readPage(db, start, limit, requestId),
  );
  return api;
}
export function renderWorkbench({
  dirty = false,
  clean = false,
  position,
  initialView = clean ? "database" : "game",
}: {
  dirty?: boolean;
  clean?: boolean;
  position?: PositionResponse;
  initialView?: "database" | "game";
} = {}) {
  const api = mockApi();
  const user = userEvent.setup();
  const view = render(
    <App
      api={api}
      initialView={initialView}
      initialState={
        clean
          ? undefined
          : {
              database,
              document: selected,
              position: position ?? {
                requestId: "initial",
                documentId: "doc-1",
                nodeId: 0,
                fen: startFen,
                turn: "white",
                legalUci: ["e2e4", "d2d4"],
              },
              dirty,
            }
      }
    />,
  );
  return { ...view, api, user };
}
