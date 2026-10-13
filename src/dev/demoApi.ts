// Browser-only demo backend for UI previews in `npm run dev` outside Tauri.
// It never ships: main.tsx imports it only in development builds.
import type { WorkbenchApi } from "../api/client";
import type { EntrySummary, EngineEventDto, GameNode } from "../api/types";
import { demoFens, demoLegal, demoNodes } from "./demoGame";

const famous: [
  string,
  string,
  string,
  string,
  string,
  string,
  number?,
  number?,
][] = [
  ["Morphy, Paul", "Duke Karl / Count Isouard", "1-0", "C41", "Paris", "1858"],
  ["Anderssen, Adolf", "Kieseritzky, Lionel", "1-0", "C33", "London", "1851"],
  ["Anderssen, Adolf", "Dufresne, Jean", "1-0", "C52", "Berlin", "1852"],
  [
    "Steinitz, William",
    "Von Bardeleben, Curt",
    "1-0",
    "C54",
    "Hastings",
    "1895",
  ],
  ["Lasker, Emanuel", "Bauer, Johann", "1-0", "A03", "Amsterdam", "1889"],
  [
    "Capablanca, Jose Raul",
    "Marshall, Frank",
    "1-0",
    "C89",
    "New York",
    "1918",
  ],
  ["Rotlewi, Georg", "Rubinstein, Akiba", "0-1", "D02", "Lodz", "1907"],
  ["Reti, Richard", "Tartakower, Savielly", "1-0", "B15", "Vienna", "1910"],
  ["Botvinnik, Mikhail", "Capablanca, Jose Raul", "1-0", "E49", "AVRO", "1938"],
  ["Byrne, Donald", "Fischer, Robert James", "0-1", "D92", "New York", "1956"],
  [
    "Kasparov, Garry",
    "Topalov, Veselin",
    "1-0",
    "B07",
    "Wijk aan Zee",
    "1999",
    2812,
    2700,
  ],
  [
    "Karpov, Anatoly",
    "Kasparov, Garry",
    "0-1",
    "B44",
    "Moscow",
    "1985",
    2720,
    2700,
  ],
  [
    "Fischer, Robert James",
    "Spassky, Boris",
    "1-0",
    "D59",
    "Reykjavik",
    "1972",
    2785,
    2660,
  ],
  [
    "Tal, Mikhail",
    "Botvinnik, Mikhail",
    "1-0",
    "B18",
    "Moscow",
    "1960",
    2580,
    2620,
  ],
  [
    "Polgar, Judit",
    "Kasparov, Garry",
    "1-0",
    "C67",
    "Moscow",
    "2002",
    2681,
    2838,
  ],
];

const entries: EntrySummary[] = Array.from({ length: 200 }, (_, i) => {
  const [white, black, result, eco, event, year, whiteElo, blackElo] =
    famous[i % famous.length];
  return {
    ordinal: String(i + 1),
    kind: i === 6 ? "text" : "game",
    deleted: i === 9,
    white,
    black,
    result,
    eco,
    event,
    date: `${year}.??.??`,
    round: String((i % 9) + 1),
    whiteElo,
    blackElo,
    diagnostics: [],
  };
});

export function demoApi(): WorkbenchApi {
  let nodes: GameNode[] = structuredClone(demoNodes) as unknown as GameNode[];
  let revision = 0;
  let timer: ReturnType<typeof setInterval> | undefined;
  const game = (e: EntrySummary) => ({
    startFen: demoFens[0],
    tags: {
      White: e.white ?? "?",
      Black: e.black ?? "?",
      Result: e.result ?? "*",
    },
    nodes,
  });
  const api: Partial<WorkbenchApi> = {
    chooseDatabase: async () => "/demo/Famous Games.cbh",
    startupRequest: async () => null,
    startupShown: async () => {},
    chooseNewDatabase: async () => null,
    confirmEditing: async () => false,
    searchGames: async (_db, _filter, requestId) => ({
      requestId,
      hits: "200",
      scanned: "2400",
      truncated: false,
      cancelled: false,
    }),
    cancelSearch: async () => {},
    findPosition: async (_db, _fen, requestId) => ({
      requestId,
      hits: "200",
      scanned: "2400",
      truncated: false,
      cancelled: false,
      indexed: false,
    }),
    createSearchBooster: async (_db, onProgress) => {
      onProgress({ done: "2400", total: "2400" });
      return true;
    },
    deleteSearchBooster: async () => {},
    // A made-up tree for previews: the first legal moves with fixed numbers.
    createOpeningTree: async (_db, onProgress) => {
      onProgress({ stage: "reading", done: "2400", total: "2400" });
      return true;
    },
    cancelOpeningTree: async () => {},
    deleteOpeningTree: async () => {},
    openingTree: async (_db, line, requestId) => {
      const legal = (demoLegal[line.moves.length] ?? []).slice(0, 5);
      const counts = [1200, 640, 310, 95, 12];
      const total = counts.slice(0, legal.length).reduce((a, b) => a + b, 0);
      return {
        requestId,
        status: "tree" as const,
        games: total,
        ended: 0,
        moves: legal.map((uci, i) => ({
          uci,
          san: uci,
          games: counts[i],
          share: (counts[i] * 100) / total,
          score: 55 - i * 3,
          draws: 30 - i * 2,
          averageElo: 2450 - i * 40,
          performance: 2480 - i * 60,
          firstYear: 1950 + i * 5,
          lastYear: 2024 - i,
        })),
      };
    },
    referenceSearch: async (_db, _line, requestId) => ({
      requestId,
      hits: "12",
      truncated: false,
      cancelled: false,
      source: "tree" as const,
      indexed: false,
    }),
    cancelReference: async () => {},
    readReferencePage: async (db, _start, _limit, requestId) => {
      const page = await api.readPage!(db, "1", 12, requestId);
      return { ...page, entries: page.entries.slice(0, 12), next: undefined };
    },
    clearSearch: async () => {},
    readSearchPage: async (db, start, limit, requestId) =>
      api.readPage!(db, start, limit, requestId),
    openDatabase: async (_path, requestId) => ({
      requestId,
      databaseId: "demo",
      entryCount: "2400",
      companions: [],
      diagnostics: [],
    }),
    readPage: async (_db, start, _limit, requestId) => ({
      requestId,
      entries: entries.map((e) => ({
        ...e,
        ordinal: String(Number(e.ordinal) + Number(start) - 1),
      })),
      next: Number(start) < 2200 ? String(Number(start) + 200) : undefined,
    }),
    loadEntry: async (_db, ordinal, requestId) => {
      const summary = entries[(Number(ordinal) - 1) % entries.length];
      return {
        requestId,
        documentId: `demo-${ordinal}`,
        revision: String(revision),
        dirty: false,
        summary: { ...summary, ordinal },
        metadataStatus: "available",
        movesStatus: "available",
        annotationsStatus: "available",
        diagnostics: [],
        game: game(summary),
      };
    },
    positionAt: async (documentId, nodeId, requestId) => ({
      requestId,
      documentId,
      nodeId,
      fen: demoFens[nodeId] ?? demoFens[0],
      turn:
        (demoFens[nodeId] ?? demoFens[0]).split(" ")[1] === "b"
          ? "black"
          : "white",
      legalUci: demoLegal[nodeId] ?? [],
    }),
    editDocument: async (documentId, edit) => {
      if (edit.kind === "setComments")
        nodes[edit.node].comments = edit.comments;
      if (edit.kind === "setNags") nodes[edit.node].nags = edit.nags;
      nodes = [...nodes];
      revision++;
      return {
        documentId,
        game: { ...game(entries[0]), nodes },
        revision: String(revision),
        dirty: true,
      };
    },
    newDocument: async (fen, requestId) => ({
      documentId: "demo-new",
      requestId,
      revision: "0",
      dirty: false,
      game: {
        startFen:
          fen ?? "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        tags: { White: "?", Black: "?", Result: "*" },
        nodes: [{ id: 0, children: [], comments: [], nags: [] }],
      },
    }),
    documentPgn: async () => '[Result "*"]\n\n*\n',
    readClipboard: async () => "",
    writeClipboard: async () => {},
    closeDatabase: async () => {},
    closeDocument: async () => {},
    watchClose: async () => () => {},
    pickEngine: async () => "/demo/stockfish",
    chooseEngine: async () => ({
      engineId: "demo-engine",
      identity: { name: "Stockfish 17 (demo)" },
      options: [
        {
          name: "Threads",
          kind: "spin",
          default: "1",
          min: 1,
          max: 64,
          vars: [],
        },
      ],
      status: "ready",
    }),
    setEngineOption: async () => {},
    startAnalysis: async (engineId, _doc, _node, requestId, onEvent) => {
      let depth = 10;
      clearInterval(timer);
      timer = setInterval(() => {
        depth++;
        const event = (
          multipv: number,
          cp: number,
          pv: string[],
        ): EngineEventDto => ({
          engineId,
          requestId,
          payload: {
            kind: "info",
            multipv,
            depth,
            nodes: String(depth * 412_337),
            score: { kind: "cp", value: cp },
            rootTurn: "white",
            pv,
            diagnostics: [],
          },
        });
        onEvent(
          event(1, 31 + (depth % 3), ["Nf3", "Nc6", "Bb5", "a6", "Ba4", "Nf6"]),
        );
        onEvent(event(2, 18, ["Bc4", "Nf6", "d3", "Bc5", "c3", "d6"]));
      }, 700);
      return () => clearInterval(timer);
    },
    stopAnalysis: async () => clearInterval(timer),
    closeEngine: async () => clearInterval(timer),
  };
  return api as WorkbenchApi;
}
