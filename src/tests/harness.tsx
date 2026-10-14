import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { vi } from "vitest";
import { App } from "../App";
import { backend } from "../api/client";
import type {
  GameResponse,
  PositionResponse,
  EntrySummary,
  AppSettings,
} from "../api/types";
export const startFen =
  "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
export const summary: EntrySummary = {
  ordinal: "1",
  kind: "game",
  deleted: false,
  white: "Alpha",
  black: "Beta",
  whiteId: 11,
  blackId: 12,
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
export function mockApi(firstDatabase = 1) {
  // Each opened database gets its own id, as the backend's do.
  let nextDatabase = firstDatabase;
  let game = structuredClone(selected.game!);
  let revision = 0;
  let settings: AppSettings = { maxGameTabs: 8, engines: [] };
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
    probeEngine: vi.fn().mockResolvedValue({ name: "Mock engine 1.0" }),
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
        } else if (edit.kind === "setMarks") {
          game.nodes[edit.node].squares = edit.squares;
          game.nodes[edit.node].arrows = edit.arrows;
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
    blunderCheck: vi
      .fn()
      .mockImplementation(
        async (
          _doc: string,
          _params: unknown,
          onProgress: (p: { done: number; total: number }) => void,
        ) => {
          onProgress({ done: 2, total: 2 });
          return {
            cancelled: false,
            checked: 2,
            revision: String(revision),
            findings: [
              {
                node: 1,
                loss: 180,
                best: "+0.30",
                played: "-1.50",
                depth: 20,
                line: ["d2d4", "d7d5"],
              },
            ],
          };
        },
      ),
    cancelBlunderCheck: vi.fn().mockResolvedValue(undefined),
    chooseTablebaseFolder: vi.fn().mockResolvedValue("/synthetic/syzygy"),
    probeTablebase: vi
      .fn()
      .mockImplementation(async (_fen: string, requestId: string) => ({
        requestId,
        status: "noTables",
        pieces: 32,
        maxPieces: 0,
        rounded: false,
        moves: [],
      })),
    getSettings: vi
      .fn()
      .mockImplementation(async () => structuredClone(settings)),
    setSettings: vi.fn().mockImplementation(async (next: AppSettings) => {
      if (next.maxGameTabs < 1 || next.maxGameTabs > 8)
        throw { message: "The most game tabs must be from 1 to 8" };
      settings = structuredClone(next);
      return next;
    }),
    chooseNewDatabase: vi.fn().mockResolvedValue("/synthetic/new.2cbh"),
    confirmEditing: vi.fn().mockResolvedValue(true),
    createDatabase: vi.fn().mockImplementation(async (_path, requestId) => ({
      ...database,
      databaseId: `db-${nextDatabase++}`,
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
    searchPosition: vi
      .fn()
      .mockImplementation(async (_db, query, requestId, onProgress) => {
        onProgress?.({ done: "200", total: "401" });
        searching = true;
        return {
          requestId,
          hits: "2",
          scanned: "401",
          truncated: false,
          cancelled: false,
          indexed: !!query.exact,
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
      .mockImplementation(async (path, requestId, writable = false) => ({
        ...database,
        databaseId: `db-${nextDatabase++}`,
        name: String(path)
          .split("/")
          .pop()!
          .replace(/\.[^.]+$/, ""),
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
    nameList: vi
      .fn()
      .mockImplementation(async (_db: string, kind: string, onProgress) => {
        onProgress({ done: "401", total: "401" });
        return { rows: kind === "players" ? "250" : "3" };
      }),
    cancelNameList: vi.fn().mockResolvedValue(undefined),
    copyRows: vi
      .fn()
      .mockImplementation(
        async (_db: string, ranges: { start: string; end: string }[]) => ({
          games: String(
            ranges.reduce((n, r) => n + Number(r.end) - Number(r.start), 0),
          ),
          truncated: false,
        }),
      ),
    pasteGames: vi.fn().mockImplementation(async (_db: string, onProgress) => {
      onProgress({ done: "3", total: "3" });
      return {
        pasted: "3",
        skipped: "0",
        incomplete: "1",
        cancelled: false,
        entryCount: "404",
        treeStale: false,
      };
    }),
    cancelPaste: vi.fn().mockResolvedValue(undefined),
    deleteRows: vi
      .fn()
      .mockImplementation(
        async (_db: string, ranges: { start: string; end: string }[]) =>
          String(
            ranges.reduce((n, r) => n + Number(r.end) - Number(r.start), 0),
          ),
      ),
    confirmPaste: vi.fn().mockResolvedValue(true),
    readNames: vi
      .fn()
      .mockImplementation(
        async (
          _db: string,
          kind: string,
          start: string,
          limit: number,
          requestId: string,
        ) => ({
          requestId,
          rows: Array.from(
            {
              length: Math.max(
                0,
                Math.min(
                  limit,
                  (kind === "players" ? 250 : 3) - Number(start) + 1,
                ),
              ),
            },
            (_, i) => {
              const n = Number(start) + i;
              return kind === "players"
                ? {
                    id: n,
                    name: `Player ${n}, A`,
                    games: n,
                    firstYear: 2000,
                    lastYear: 2000 + (n % 5),
                  }
                : {
                    id: n,
                    name: `Open ${n}`,
                    site: "Oslo",
                    games: 10 * n,
                    firstYear: 2020,
                    lastYear: 2020,
                  };
            },
          ),
        }),
      ),
    findName: vi.fn().mockResolvedValue("120"),
    playerStatistics: vi
      .fn()
      .mockImplementation(
        async (
          _db: string,
          playerId: number,
          onProgress: (p: { done: string; total: string }) => void,
        ) => {
          onProgress({ done: "50", total: "100" });
          const score = (games: number, wins: number, draws: number) => ({
            games,
            wins,
            draws,
            losses: games - wins - draws,
            points: wins + draws / 2,
            percent: Math.round(((wins + draws / 2) * 100) / games),
            averageElo: 2500,
            performance: 2600,
          });
          return {
            playerId,
            name: `Player ${playerId}, A`,
            total: score(3, 2, 1),
            white: score(2, 2, 0),
            black: score(1, 0, 1),
            found: 3,
            deleted: 0,
            unscored: 0,
            firstYear: 2019,
            lastYear: 2020,
            openings: [
              { eco: "B90", white: score(2, 2, 0), black: score(0, 0, 0) },
              { eco: "C42", white: score(0, 0, 0), black: score(1, 0, 1) },
            ],
            opponents: [
              {
                id: 5,
                name: "Bravo, B",
                elo: 2450,
                score: score(2, 1, 1),
                lastYear: 2020,
              },
              {
                id: 6,
                name: "Alpha, C",
                elo: 2550,
                score: score(1, 1, 0),
                lastYear: 2019,
              },
            ],
            tournaments: [
              {
                id: 9,
                name: "Open 9",
                site: "Oslo",
                year: 2020,
                score: score(2, 1, 1),
              },
              {
                id: 8,
                name: "Club",
                site: "Bergen",
                year: 2019,
                score: score(1, 1, 0),
              },
            ],
            years: [
              { year: 2019, score: score(1, 1, 0), elo: 2400 },
              { year: 2020, score: score(2, 1, 1), elo: 2450 },
            ],
            truncated: false,
            moreOpponents: 0,
            moreTournaments: 0,
          };
        },
      ),
    cancelPlayerStatistics: vi.fn().mockResolvedValue(undefined),
    statisticsGames: vi
      .fn()
      .mockImplementation(
        async (_db: string, _p: number, _g: unknown, requestId: string) => ({
          requestId,
          hits: "2",
          scanned: "2",
          truncated: false,
          cancelled: false,
        }),
      ),
    crosstable: vi
      .fn()
      .mockImplementation(
        async (_db: string, tournamentId: number, layout) => ({
          event: `Open ${tournamentId}`,
          site: "Oslo",
          layout: layout ?? "roundRobin",
          rounds: 1,
          games: 1,
          unscored: 0,
          truncated: false,
          averageElo: 2400,
          category: 6,
          players: [
            {
              id: 1,
              name: "Alpha, A",
              elo: 2450,
              points: 1,
              games: 1,
              wins: 1,
              blackGames: 0,
              blackWins: 0,
              sonnebornBerger: 0,
              buchholz: 0,
              performance: 2750,
              cells:
                layout === "swiss"
                  ? [
                      [
                        {
                          ordinal: 7,
                          mark: "1",
                          white: true,
                          round: 1,
                          opponent: 1,
                        },
                      ],
                    ]
                  : [
                      [],
                      [
                        {
                          ordinal: 7,
                          mark: "1",
                          white: true,
                          round: 1,
                          opponent: 1,
                        },
                      ],
                    ],
            },
            {
              id: 2,
              name: "Beta, B",
              elo: 2350,
              points: 0,
              games: 1,
              wins: 0,
              blackGames: 1,
              blackWins: 0,
              sonnebornBerger: 0,
              buchholz: 1,
              performance: 2050,
              cells:
                layout === "swiss"
                  ? [
                      [
                        {
                          ordinal: 7,
                          mark: "0",
                          white: false,
                          round: 1,
                          opponent: 0,
                        },
                      ],
                    ]
                  : [
                      [
                        {
                          ordinal: 7,
                          mark: "0",
                          white: false,
                          round: 1,
                          opponent: 0,
                        },
                      ],
                      [],
                    ],
            },
          ],
        }),
      ),
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
  const api = mockApi(clean ? 1 : 2);
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
