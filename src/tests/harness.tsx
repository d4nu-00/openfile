import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { vi } from "vitest";
import { App } from "../App";
import { backend } from "../api/client";
import type { ActiveSearch } from "../state/workbench";
import type {
  GameResponse,
  PositionResponse,
  EntrySummary,
  AppSettings,
  RepertoireProgress,
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
export function mockApi(
  firstDatabase = 1,
  initialSettings: Partial<AppSettings> = {},
) {
  // Each opened database gets its own id, as the backend's do.
  let nextDatabase = firstDatabase;
  let game = structuredClone(selected.game!);
  let revision = 0;
  let settings: AppSettings = {
    maxGameTabs: 8,
    engines: [],
    ...initialSettings,
  };
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
        } else if (edit.kind === "setTraining") {
          const n = game.nodes[edit.node];
          n.training = edit.training ?? undefined;
          n.trainingMoves = edit.training ? edit.moves : undefined;
          revision++;
        } else if (edit.kind === "setExtra") {
          const n = game.nodes[edit.node];
          const x = n.extras ?? {
            medal: 0,
            diagram: false,
            pawnStructure: false,
          };
          const e = edit.extra;
          if (e.kind === "medal") x.medal = e.bits;
          else if (e.kind === "diagram") x.diagram = e.on;
          else if (e.kind === "pawnStructure") x.pawnStructure = e.on;
          else if (e.kind === "piecePath")
            x.piecePath = e.on ? n.uci?.slice(2, 4) : undefined;
          else if (e.kind === "moveTime")
            x.moveTime =
              e.time === null
                ? undefined
                : `${e.time >>> 24}:${String((e.time >>> 16) & 255).padStart(2, "0")}:${String((e.time >>> 8) & 255).padStart(2, "0")}`;
          n.extras = x;
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
            evaluations: [
              { node: 1, nag: 17 },
              { node: 2, nag: 11 },
            ],
          };
        },
      ),
    cancelBlunderCheck: vi.fn().mockResolvedValue(undefined),
    centipawnAnalysis: vi
      .fn()
      .mockImplementation(
        async (
          _doc: string,
          _params: unknown,
          onProgress: (p: { done: number; total: number }) => void,
        ) => {
          onProgress({ done: 3, total: 4 });
          const side = (loss: number, mistakes: number) => ({
            moves: 2,
            averageLoss: loss,
            accuracy: 80,
            errorValue: loss / 2,
            categories: [0, 0, 1, 0, 0, mistakes, 0, 0],
          });
          return {
            revision: "0",
            cancelled: false,
            positions: 4,
            bookPlies: 10,
            millisPerMove: 750,
            white: side(18, 1),
            black: side(40, 0),
            items: [
              { node: 0, comment: "Centipawn Analysis after ply 10" },
              { node: 1, comment: "Mistake (−1.2)" },
            ],
            moves: [
              {
                node: 1,
                ply: 1,
                moveNumber: 1,
                san: "e4",
                white: true,
                loss: 120,
                category: "mistake",
                bestUci: "d2d4",
                bestLine: ["d4", "d5"],
              },
            ],
          };
        },
      ),
    cancelCentipawnAnalysis: vi.fn().mockResolvedValue(undefined),
    instantAnalysis: vi.fn().mockResolvedValue({
      revision: "0",
      pass: 1,
      millisPerPosition: 105,
      done: 0,
      total: 0,
      profile: [],
      finished: true,
      cancelled: false,
    }),
    cancelInstantAnalysis: vi.fn().mockResolvedValue(undefined),
    centipawnList: vi.fn().mockResolvedValue({
      cancelled: false,
      error: null,
      games: 0,
      analysed: 0,
      replaced: 0,
      skippedCount: 0,
      skipped: [],
      millisPerMove: 750,
      players: [],
      csv: "",
    }),
    cancelCentipawnList: vi.fn().mockResolvedValue(undefined),
    findNovelty: vi.fn().mockResolvedValue({
      novelty: null,
      knownPly: 0,
      total: 0,
      games: [],
      queries: 0,
      notSearched: false,
      cancelled: false,
    }),
    cancelFindNovelty: vi.fn().mockResolvedValue(undefined),
    chooseCsv: vi.fn().mockResolvedValue("/synthetic/stats.csv"),
    saveCsv: vi.fn().mockResolvedValue(undefined),
    deepAnalysis: vi
      .fn()
      .mockImplementation(
        async (
          _doc: string,
          params: { node: number },
          onProgress: (p: unknown) => void,
        ) => {
          onProgress({
            iteration: 1,
            positions: 7,
            nodes: 9,
            elapsedMs: 4000,
            best: ["d4", "d5"],
            score: "+0.20",
          });
          return {
            cancelled: false,
            iterations: 1,
            positions: 7,
            limited: false,
            node: params.node,
            revision: String(revision),
            best: ["d4", "d5"],
            score: "+0.20",
            lines: [
              { moves: ["d2d4", "d7d5"], evaluation: 11, comment: "+0.20/18" },
              { moves: ["c2c4"], evaluation: 14, comment: "+0.35/18" },
            ],
          };
        },
      ),
    stopDeepAnalysis: vi.fn().mockResolvedValue(undefined),
    openingReport: vi
      .fn()
      .mockImplementation(
        async (
          _db: string,
          _line: unknown,
          onProgress: (p: {
            stage: string;
            done: string;
            total: string;
          }) => void,
        ) => {
          onProgress({
            stage: "Reading the games' data",
            done: "5",
            total: "60",
          });
          return {
            eco: "B20",
            name: "Sicilian Defense",
            line: ["e4", "c5"],
            whiteToMove: true,
            games: 60,
            truncated: false,
            cancelled: false,
            whiteWins: 30,
            draws: 20,
            blackWins: 10,
            otherResults: 0,
            listed: [],
            earliest: {
              ordinal: "3",
              white: "Old, O",
              black: "Older, P",
              date: "1851.01.01",
              result: "1-0",
            },
            latest: {
              ordinal: "9",
              white: "New, N",
              black: "Newer, M",
              whiteElo: 2600,
              date: "2024.05.01",
              result: "1/2-1/2",
            },
            years: [
              [1851, 1],
              [2024, 59],
            ],
            fashion: [
              { from: 1850, to: 1854, games: 1, index: 40 },
              { from: 2020, to: 2024, games: 59, index: 180 },
            ],
            strongWhite: [
              {
                name: "Strong, S",
                games: 12,
                score: 75,
                bestElo: 2750,
                averageElo: 2700,
              },
            ],
            strongBlack: [],
            notable: [
              { name: "Busy, B", games: 15, score: 40, averageElo: 2300 },
            ],
            moves: [
              {
                uci: "g1f3",
                san: "Nf3",
                games: 40,
                share: 66.7,
                score: 55,
                draws: 30,
                averageElo: 2400,
                performance: 2450,
                lastYear: 2024,
                playedBy: ["Strong, S"],
              },
              {
                uci: "b1c3",
                san: "Nc3",
                games: 20,
                share: 33.3,
                score: 45,
                draws: 25,
                playedBy: [],
              },
            ],
            movesFromTree: true,
            read: 60,
            mainLine: ["Nf3", "d6", "d4"],
          };
        },
      ),
    classifyGame: vi.fn().mockResolvedValue({
      eco: "C20",
      name: "King's Pawn Game",
      ply: 2,
      current: "C40",
    }),
    setEcoCodes: vi
      .fn()
      .mockImplementation(
        async (
          _db: string,
          _only: boolean,
          onProgress: (p: { done: string; total: string }) => void,
        ) => {
          onProgress({ done: "200", total: "401" });
          return {
            games: "399",
            changed: "12",
            unchanged: "300",
            kept: "80",
            unclassified: "7",
            unreadable: "0",
            agreed: "300",
            hadCode: "380",
            cancelled: false,
          };
        },
      ),
    analysisJobs: vi.fn().mockResolvedValue({ jobs: [], running: false }),
    addAnalysisJobs: vi
      .fn()
      .mockImplementation(async (databaseId, ranges, kind) => ({
        view: {
          running: false,
          jobs: ranges.map((r: { start: string }, i: number) => ({
            id: i + 1,
            databaseId,
            ordinal: r.start,
            label: "White – Black",
            moves: ["e2e4", "e7e5"],
            kind,
            status: "waiting",
          })),
        },
        leftOut: 0,
      })),
    addAnalysisJob: vi.fn().mockResolvedValue({ jobs: [], running: false }),
    editAnalysisJob: vi.fn().mockResolvedValue({ jobs: [], running: false }),
    clearAnalysisJobs: vi.fn().mockResolvedValue({ jobs: [], running: false }),
    runAnalysisJobs: vi.fn().mockResolvedValue({
      stopped: false,
      done: 1,
      skipped: 0,
      failed: 0,
      jobs: [],
    }),
    stopAnalysisJobs: vi.fn().mockResolvedValue(undefined),
    chooseTablebaseFolder: vi.fn().mockResolvedValue("/synthetic/syzygy"),
    startScreen: vi.fn().mockResolvedValue({
      favorites: [
        {
          path: "/dbs/test database.cbh",
          name: "test database",
          folder: "/dbs",
          kind: "large",
          favorite: true,
          exists: true,
          format: "classic",
          entries: 11151119,
        },
        {
          path: "/dbs/Gone.2cbh",
          name: "Gone",
          folder: "/dbs",
          kind: "unspecific",
          favorite: true,
          exists: false,
        },
      ],
      recent: [
        {
          path: "/work/Openings.2cbh",
          name: "Openings",
          folder: "/work",
          kind: "unspecific",
          favorite: false,
          exists: true,
          format: "new",
          entries: 120,
        },
      ],
      folders: [{ path: "/work", name: "work", exists: true }],
      kinds: [
        "unspecific",
        "work",
        "large",
        "openings",
        "informant",
        "classical",
        "correspondence",
        "tactics",
        "analysis",
        "training",
        "endings",
        "studies",
        "problems",
        "myGames",
        "computer",
        "magazine",
        "patzer",
        "match",
        "recentTournaments",
        "important",
      ],
    }),
    folderDatabases: vi.fn().mockResolvedValue({
      symbols: [
        {
          path: "/work/Openings.2cbh",
          name: "Openings",
          folder: "/work",
          kind: "unspecific",
          favorite: false,
          exists: true,
          format: "new",
          entries: 120,
        },
      ],
      truncated: false,
    }),
    pinDatabase: vi.fn().mockResolvedValue(undefined),
    unpinDatabase: vi.fn().mockResolvedValue(undefined),
    setDatabaseKind: vi.fn().mockResolvedValue(undefined),
    setDatabaseProperties: vi.fn().mockResolvedValue(undefined),
    setDatabaseView: vi.fn().mockResolvedValue(undefined),
    chooseRecoveryDestination: vi.fn().mockResolvedValue("/scratch/Recovered"),
    beginRecovery: vi.fn().mockResolvedValue("recovery-test"),
    cancelRecovery: vi.fn().mockResolvedValue(undefined),
    recoverDatabase: vi.fn(),
    forgetRecent: vi.fn().mockResolvedValue(undefined),
    addFolderShortcut: vi.fn().mockResolvedValue(undefined),
    removeFolderShortcut: vi.fn().mockResolvedValue(undefined),
    chooseFolder: vi.fn().mockResolvedValue("/new/folder"),
    tablebaseAnnotation: vi.fn().mockResolvedValue({
      revision: "0",
      items: [
        {
          node: 1,
          nag: 2,
          comment: "Tablebase: draw",
          line: ["d2d4"],
          lineComment: "Tablebase: White wins",
        },
      ],
      probed: 2,
      entered: 1,
      mistakes: 1,
      status: "found",
    }),
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
    cloudStatus: vi.fn().mockResolvedValue({ files: [], bytes: "0" }),
    confirmCloudDownload: vi.fn().mockResolvedValue(true),
    downloadCloudFiles: vi.fn(
      async (
        _path: string,
        _request: string,
        onProgress: (p: {
          filesDone: number;
          files: number;
          bytesDone: string;
          bytes: string;
        }) => void,
      ) => {
        onProgress({
          filesDone: 2,
          files: 2,
          bytesDone: "1500000000",
          bytes: "1500000000",
        });
      },
    ),
    cancelCloudDownload: vi.fn().mockResolvedValue(undefined),
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
    filteredOpeningTree: vi
      .fn()
      .mockImplementation(async (_db, _line, requestId) => ({
        requestId,
        status: "filtered",
        games: 2,
        ended: 0,
        sampledHits: 3,
        totalHits: 3,
        moves: [
          { uci: "e2e4", san: "e4", games: 2, share: 100, score: 75, draws: 0 },
        ],
      })),
    cancelFilteredOpeningTree: vi.fn().mockResolvedValue(undefined),
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
    referenceStatistics: vi
      .fn()
      .mockImplementation(async (_db, _line, requestId) => ({
        requestId,
        fullGames: 10,
        filteredGames: 7,
        playerGames: 2,
        scannedGames: 10,
        afterElo: 8,
        afterYear: 7,
        missingElo: 1,
        missingYear: 2,
        complete: true,
        fullComplete: true,
        moves: [
          { san: "e4", full: 6, filtered: 4, player: 2 },
          { san: "d4", full: 3, filtered: 2, player: 0 },
        ],
      })),
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
    chooseConvertTarget: vi.fn().mockResolvedValue("/dbs/Converted.2cbh"),
    convertEstimate: vi.fn().mockResolvedValue({
      games: "404",
      bytes: "800000",
      free: "5000000000",
      fits: true,
    }),
    convertDatabase: vi
      .fn()
      .mockImplementation(async (_db: string, _ranges, _target, onProgress) => {
        onProgress({ done: "404", total: "404" });
        return {
          done: 404,
          written: 400,
          deleted: 2,
          incomplete: 3,
          texts: 4,
          chess960: 0,
          unreadable: 0,
          cancelled: false,
          path: "/dbs/Converted.2cbh",
        };
      }),
    cancelConvert: vi.fn().mockResolvedValue(undefined),
    checkIntegrity: vi
      .fn()
      .mockImplementation(async (_db: string, onProgress) => {
        onProgress({ done: "404", total: "404" });
        return {
          checked: 404,
          games: 400,
          texts: 4,
          deleted: 1,
          counts: { damagedMoves: 1 },
          problems: [
            { ordinal: 7, code: "damagedMoves", message: "Block checksum" },
          ],
          cancelled: false,
        };
      }),
    findDoubleGames: vi
      .fn()
      .mockImplementation(async (_db: string, _options, onProgress) => {
        onProgress({ done: "404", total: "404" });
        return {
          checked: "404",
          marked: "2",
          keptBoth: "1",
          pairs: [
            ["3", "2"],
            ["9", "8"],
          ],
          cancelled: false,
          treeStale: false,
        };
      }),
    removeDeletedGames: vi
      .fn()
      .mockImplementation(async (_db: string, onProgress) => {
        onProgress({ done: "404", total: "404" });
        return {
          kept: "400",
          removed: "4",
          bytesBefore: "900000",
          bytesAfter: "800000",
          cancelled: false,
          entryCount: "400",
          moved: [],
          treeDeleted: false,
        };
      }),
    cancelMaintenance: vi.fn().mockResolvedValue(undefined),
    markRows: vi
      .fn()
      .mockImplementation(async (_db: string, _ranges, onProgress) => {
        onProgress({ done: "3", total: "3" });
        return { changed: "3", marked: "2", cancelled: false };
      }),
    setNamesOfRows: vi
      .fn()
      .mockImplementation(async (_db: string, _ranges, _names, onProgress) => {
        onProgress({ done: "3", total: "3" });
        return { changed: "3", marked: "0", cancelled: false };
      }),
    confirmRemoveDeleted: vi.fn().mockResolvedValue(true),
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
    prepareAgainst: vi
      .fn()
      .mockImplementation(
        async (
          _db: string,
          playerId: number,
          white: boolean,
          onProgress: (p: {
            stage: string;
            done: string;
            total: string;
          }) => void,
        ) => {
          onProgress({ stage: "lines", done: "1", total: "2" });
          return {
            playerId,
            name: `Player ${playerId}, A`,
            white,
            games: 3,
            deleted: 0,
            read: 3,
            unreadable: 0,
            truncated: false,
            score: {
              games: 3,
              wins: 1,
              draws: 1,
              losses: 1,
              points: 1.5,
              percent: 50,
              averageElo: 2500,
              performance: 2500,
            },
            recentRating: 2450,
            firstYear: 2019,
            lastYear: 2020,
            repertoire: [
              {
                moves: ["e2e4", "c7c5", "g1f3"],
                text: "1.e4 c5 2.Nf3",
                eco: "B27",
                name: "Sicilian Defense",
                games: 2,
                score: 75,
                lastYear: 2020,
              },
            ],
            openings: [
              {
                eco: "B27",
                score: {
                  games: 2,
                  wins: 1,
                  draws: 1,
                  losses: 0,
                  points: 1.5,
                  percent: 75,
                },
              },
            ],
            latest: [
              {
                ordinal: "3",
                year: 2020,
                opponent: "Bravo, B",
                result: "0-1",
                eco: "D02",
                tournament: "Club",
              },
            ],
            lastGame: "3",
          };
        },
      ),
    cancelPreparation: vi.fn().mockResolvedValue(undefined),
    chooseRepertoireFolder: vi.fn().mockResolvedValue("/synthetic/rep"),
    chooseRepertoireDatabase: vi
      .fn()
      .mockResolvedValue("/synthetic/rep/Repertoire White.2cbh"),
    frequentPlayers: vi.fn().mockResolvedValue([
      { id: 7, name: "Alpha, A", games: 5 },
      { id: 8, name: "Bravo, B", games: 3 },
    ]),
    generateRepertoire: vi
      .fn()
      .mockImplementation(
        async (
          _db: string,
          players: number[],
          folder: string,
          onProgress: (p: RepertoireProgress) => void,
        ) => {
          onProgress({ stage: "lines", done: "1", total: "2" });
          const side = (white: boolean) => ({
            white,
            path: `${folder}/Repertoire ${white ? "White" : "Black"}.2cbh`,
            games: 3,
            read: 3,
            truncated: false,
            repertoireGames: white ? 1 : 0,
            merged: white ? 2 : 0,
            unclassified: 0,
            rare: 1,
            cut: 0,
          });
          const black = { ...side(false), path: null };
          return {
            players: players.map((id) => (id === 7 ? "Alpha, A" : "?")),
            sides: [side(true), black],
          };
        },
      ),
    repertoireReport: vi
      .fn()
      .mockImplementation(
        async (
          _db: string,
          repertoire: string,
          onProgress: (p: RepertoireProgress) => void,
        ) => {
          onProgress({ stage: "scan", done: "5", total: "10" });
          return {
            repertoire: repertoire.split("/").pop(),
            repertoireGames: 1,
            repertoireTruncated: false,
            positions: 1,
            scanned: "10",
            scanTruncated: false,
            matched: "2",
            topics: [
              {
                name: "Sicilian Defense (Najdorf Variation, English Attack)",
                eco: "B90",
                positions: [
                  {
                    text: "1.e4 c5 2.Nf3 d6 3.d4 cxd4 4.Nxd4 Nf6 5.Nc3 a6 6.Be3",
                    fen: "rnbqkb1r/1p2pppp/p2p1n2/8/3NP3/2N1B3/PPP2PPP/R2QKB1R b KQkq - 1 6",
                    games: 2,
                    ordinals: ["2", "1"],
                    top: [
                      {
                        ordinal: "2",
                        white: "Delta, D",
                        black: "Echo, E",
                        whiteElo: 2700,
                        blackElo: 2700,
                        year: 2020,
                        result: "1-0",
                      },
                    ],
                  },
                ],
              },
            ],
            positionsTruncated: false,
            cancelled: false,
          };
        },
      ),
    cancelRepertoire: vi.fn().mockResolvedValue(undefined),
    playerDossier: vi
      .fn()
      .mockImplementation(
        async (
          _db: string,
          playerId: number,
          onProgress: (p: {
            stage: string;
            done: string;
            total: string;
          }) => void,
        ) => {
          onProgress({ stage: "mates", done: "1", total: "2" });
          // Missing values arrive as null, as the backend sends them.
          const none = null as unknown as undefined;
          const colour = (score?: number, band?: number) => ({
            games: 3,
            whiteWins: 2,
            draws: 1,
            blackWins: 0,
            score: score ?? none,
            scoreBand: band ?? none,
            opponentsAverage: score === undefined ? none : 2500,
            performance: score === undefined ? none : 2620,
            whiteWinMoves: 30,
            whiteWinBand: 1,
            drawMoves: 15,
            drawBand: 0,
            drawPercent: 33,
            drawQuoteBand: 2,
            quickDrawPercent: 33,
          });
          const game = (ordinal: string, medals: string[] = []) => ({
            ordinal,
            white: true,
            opponent: "Bravo, B",
            year: 2020,
            result: "1-0",
            moves: 30,
            eco: "C42",
            tournament: "Club",
            medals,
          });
          return {
            playerId,
            name: `Player ${playerId}, A`,
            games: 3,
            deleted: 0,
            truncated: false,
            firstYear: 2019,
            lastYear: 2020,
            years: [
              {
                year: 2020,
                elo: 2450,
                score: {
                  games: 3,
                  wins: 2,
                  draws: 1,
                  losses: 0,
                  points: 2.5,
                  percent: 83,
                },
              },
            ],
            all: colour(),
            white: colour(83, 6),
            black: colour(),
            ratingBands: [{ from: 2500, games: 2, points: 1.5, percent: 75 }],
            tournaments: [],
            opponents: [
              {
                id: 5,
                name: "Bravo, B",
                elo: 2450,
                score: {
                  games: 2,
                  wins: 1,
                  draws: 1,
                  losses: 0,
                  points: 1.5,
                  percent: 75,
                },
                lastYear: 2020,
              },
            ],
            moreTournaments: 0,
            moreOpponents: 0,
            repertoireWhite: [
              {
                moves: ["e2e4"],
                text: "1.e4",
                eco: "B00",
                name: "King's Pawn",
                games: 3,
                score: 83,
                lastYear: 2020,
              },
            ],
            repertoireBlack: [],
            medalGames: [game("2", ["Best game"])],
            medalCount: 1,
            matesGiven: [game("3")],
            matesGivenCount: 1,
            matesReceived: [],
            matesReceivedCount: 0,
            matesRead: 3,
            annotated: [],
            annotatedCount: 0,
          };
        },
      ),
    playingStyle: vi
      .fn<typeof backend.playingStyle>()
      .mockResolvedValue(undefined),
    cancelPlayingStyle: vi.fn().mockResolvedValue(undefined),
    validatePlayingStyle: vi.fn().mockResolvedValue(undefined),
    loadStyleEntry: vi
      .fn<typeof backend.loadStyleEntry>()
      .mockImplementation((db, _job, ordinal, request): Promise<GameResponse> =>
        api.loadEntry(db, ordinal, request),
      ),
    cancelDossier: vi.fn().mockResolvedValue(undefined),
    closePreparation: vi.fn().mockResolvedValue(undefined),
    preparedTree: vi
      .fn()
      .mockImplementation(async (_db: string, _line: unknown, requestId) => ({
        requestId,
        status: "tree",
        games: 3,
        ended: 0,
        moves: [
          {
            uci: "e2e4",
            san: "e4",
            games: 2,
            share: 66.7,
            score: 75,
            draws: 50,
            firstYear: 2019,
            lastYear: 2020,
          },
        ],
      })),
    preparationGames: vi
      .fn()
      .mockImplementation(
        async (
          _db: string,
          _p: number,
          _w: boolean,
          _l: unknown,
          requestId: string,
        ) => ({
          requestId,
          hits: "2",
          scanned: "3",
          truncated: false,
          cancelled: false,
        }),
      ),
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
    tournamentReport: vi
      .fn()
      .mockImplementation(async (_db: string, tournamentId: number) => {
        const player = (id: number, name: string, pts: number) => ({
          id,
          name,
          elo: 2400,
          points: pts,
          games: 2,
          wins: 1,
          blackGames: 1,
          blackWins: 0,
          sonnebornBerger: 0.5,
          buchholz: 1,
          performance: 2500,
          cells: [],
        });
        return {
          table: {
            event: `Open ${tournamentId}`,
            site: "Oslo",
            layout: "swiss",
            rounds: 2,
            games: 3,
            unscored: 0,
            truncated: false,
            averageElo: 2400,
            category: null,
            players: [
              player(1, "Alpha, A", 1.5),
              player(2, "Bravo, B", 1),
              player(3, "Charlie, C", 0.5),
            ],
          },
          rounds: [
            {
              round: 1,
              date: "2020.01.10",
              games: [
                { ordinal: 7, white: 1, black: 0, result: "0-1", moves: 30 },
              ],
              standings: [
                { row: 0, points: 1, games: 1 },
                { row: 1, points: 0, games: 1 },
              ],
            },
            {
              round: 2,
              date: "2020.??.??",
              games: [
                {
                  ordinal: 8,
                  white: 0,
                  black: 2,
                  result: "1/2-1/2",
                  moves: 41,
                },
              ],
              standings: [{ row: 0, points: 1.5, games: 2 }],
            },
          ],
          stats: {
            games: 3,
            whiteWins: 1,
            draws: 1,
            blackWins: 1,
            forfeits: 0,
            whiteScore: 50,
            averageMoves: 35,
          },
        };
      }),
    selectionPlayers: vi.fn().mockResolvedValue([
      { id: 3, name: "Player 3, A", games: 2 },
      { id: 5, name: "Bravo, B", games: 1 },
    ]),
    errorReport: vi
      .fn()
      .mockImplementation(
        async (
          _db: string,
          params: { player: number | null; slower: boolean },
          onProgress: (p: {
            gamesDone: number;
            games: number;
            positions: number;
          }) => void,
        ) => {
          onProgress({ gamesDone: 1, games: 2, positions: 40 });
          return {
            cancelled: false,
            depth: params.slower ? 12 : 9,
            games: 2,
            skipped: 0,
            truncated: false,
            player: params.player ? "Player 3, A" : null,
            moves: 60,
            averageLoss: 18.5,
            errorValue: 17.2,
            pliesPerGame: 84,
            blunderElo: 2190,
            rating: 2400,
            difference: -210,
            severity: [6, 3, 2, 1],
            distribution: [6, 2, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0],
            phases: {
              opening: { moves: 20, errors: 1, averageLoss: 8 },
              middlegame: { moves: 40, errors: 5, averageLoss: 24 },
            },
            opponents: { equal: { moves: 60, errors: 6, averageLoss: 18.5 } },
            transitions: [
              [0, 1, 0, 0, 0],
              [0, 0, 2, 0, 0],
              [0, 0, 1, 2, 0],
              [0, 0, 0, 0, 0],
              [0, 0, 0, 0, 0],
            ],
            worst: [
              {
                ordinal: "3",
                node: 21,
                number: "11.",
                san: "Qxb7",
                best: "Nf3",
                loss: 340,
              },
            ],
          };
        },
      ),
    cancelErrorReport: vi.fn().mockResolvedValue(undefined),
    showEntries: vi
      .fn()
      .mockImplementation(
        async (_db: string, ordinals: string[], requestId: string) => ({
          requestId,
          hits: String(ordinals.length),
          scanned: String(ordinals.length),
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
  search,
  settings,
}: {
  dirty?: boolean;
  clean?: boolean;
  position?: PositionResponse;
  initialView?: "database" | "game";
  search?: ActiveSearch;
  settings?: Partial<AppSettings>;
} = {}) {
  const api = mockApi(clean ? 1 : 2, settings);
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
              search,
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
