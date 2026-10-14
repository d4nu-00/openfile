// Browser-only demo backend for UI previews in `npm run dev` outside Tauri.
// It never ships: main.tsx imports it only in development builds.
import type { WorkbenchApi } from "../api/client";
import type {
  EntrySummary,
  EngineEventDto,
  GameNode,
  NameKind,
  NameRow,
  SortColumn,
  AppSettings,
  PlayerStatistics,
  StatScore,
} from "../api/types";
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

const demoCount = 2400;
const demoEntry = (n: number): EntrySummary => ({
  ...entries[(n - 1) % entries.length],
  ordinal: String(n),
});
const demoKey: Record<SortColumn, (e: EntrySummary) => string | number> = {
  white: (e) => e.white ?? "",
  whiteElo: (e) => e.whiteElo ?? 0,
  black: (e) => e.black ?? "",
  blackElo: (e) => e.blackElo ?? 0,
  result: (e) => e.result ?? "",
  eco: (e) => e.eco ?? "",
  tournament: (e) => e.event ?? "",
  round: (e) => Number(e.round ?? 0),
  date: (e) => e.date ?? "",
  annotator: (e) => e.annotator ?? "",
  source: (e) => e.source ?? "",
  moves: () => 0,
};

function demoNames(kind: NameKind): NameRow[] {
  const names =
    kind === "players"
      ? entries.flatMap((e) => [e.white!, e.black!])
      : entries.map((e) => e.event!);
  const counts = new Map<string, number>();
  for (const n of names) counts.set(n, (counts.get(n) ?? 0) + 12);
  return [...counts.keys()].sort().map((name, id) => ({
    id,
    name,
    games: counts.get(name)!,
    firstYear: 1851,
    lastYear: 2002,
  }));
}

/** Statistics of a demo player, scored from the sample entries. */
function demoStatistics(playerId: number): PlayerStatistics {
  const name = demoNames("players")[playerId]?.name;
  const games = entries.filter(
    (e) =>
      e.kind === "game" && !e.deleted && (e.white === name || e.black === name),
  );
  const empty = (): StatScore => ({
    games: 0,
    wins: 0,
    draws: 0,
    losses: 0,
    points: 0,
  });
  const add = (s: StatScore, e: EntrySummary) => {
    const white = e.white === name;
    const half =
      e.result === "1/2-1/2" ? 1 : (e.result === "1-0") === white ? 2 : 0;
    s.games++;
    s.points += half / 2;
    if (half === 2) s.wins++;
    else if (half === 1) s.draws++;
    else s.losses++;
    s.percent = Math.round((s.points * 100) / s.games);
  };
  const total = empty();
  const white = empty();
  const black = empty();
  const openings = new Map<string, { white: StatScore; black: StatScore }>();
  const opponents = new Map<string, StatScore>();
  const events = new Map<string, StatScore>();
  const years = new Map<number, StatScore>();
  for (const e of games) {
    const w = e.white === name;
    add(total, e);
    add(w ? white : black, e);
    const o = openings.get(e.eco ?? "") ?? { white: empty(), black: empty() };
    add(w ? o.white : o.black, e);
    openings.set(e.eco ?? "", o);
    const opp = (w ? e.black : e.white) ?? "?";
    const os = opponents.get(opp) ?? empty();
    add(os, e);
    opponents.set(opp, os);
    const es = events.get(e.event ?? "?") ?? empty();
    add(es, e);
    events.set(e.event ?? "?", es);
    const year = Number(e.date?.slice(0, 4)) || 0;
    const ys = years.get(year) ?? empty();
    add(ys, e);
    years.set(year, ys);
  }
  const playerIds = demoNames("players");
  const eventIds = demoNames("tournaments");
  return {
    playerId,
    name,
    total,
    white,
    black,
    found: games.length,
    deleted: 0,
    unscored: 0,
    firstYear: Math.min(...years.keys()),
    lastYear: Math.max(...years.keys()),
    openings: [...openings].map(([eco, o]) => ({
      eco: eco || undefined,
      ...o,
    })),
    opponents: [...opponents].map(([n, score]) => ({
      id: playerIds.findIndex((p) => p.name === n),
      name: n,
      score,
      lastYear: 2000,
    })),
    tournaments: [...events].map(([n, score]) => ({
      id: eventIds.findIndex((t) => t.name === n),
      name: n,
      year: 2000,
      score,
    })),
    years: [...years].map(([year, score]) => ({ year, score })),
    truncated: false,
    moreOpponents: 0,
    moreTournaments: 0,
  };
}

export function demoApi(): WorkbenchApi {
  // The list: a search's hits and a sort order, both over entry numbers.
  let hits: number[] | undefined;
  let order: number[] | undefined;
  const all = () => Array.from({ length: demoCount }, (_, i) => i + 1);
  const list = () => {
    const base = hits ?? all();
    if (!order) return base;
    const keep = new Set(base);
    return order.filter((o) => keep.has(o));
  };
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
  let settings: AppSettings = { maxGameTabs: 8, engines: [] };
  const api: Partial<WorkbenchApi> = {
    chooseDatabase: async () => "/demo/Famous Games.cbh",
    startupRequest: async () => null,
    startupShown: async () => {},
    getSettings: async () => structuredClone(settings),
    chooseTablebaseFolder: async () => null,
    blunderCheck: async () => ({
      cancelled: false,
      checked: 0,
      findings: [],
      revision: "0",
    }),
    cancelBlunderCheck: async () => {},
    startScreen: async () => ({
      favorites: [
        {
          path: "/demo/Mega Database.cbh",
          name: "Mega Database",
          folder: "/demo",
          kind: "large",
          favorite: true,
          exists: true,
          format: "classic",
          entries: 2400,
        },
      ],
      recent: [],
      folders: [],
      kinds: ["unspecific", "large"],
    }),
    folderDatabases: async () => ({ symbols: [], truncated: false }),
    pinDatabase: async () => {},
    unpinDatabase: async () => {},
    setDatabaseKind: async () => {},
    forgetRecent: async () => {},
    addFolderShortcut: async () => {},
    removeFolderShortcut: async () => {},
    chooseFolder: async () => null,
    tablebaseAnnotation: async () => ({
      revision: "0",
      items: [],
      probed: 0,
      mistakes: 0,
      status: "noTables",
    }),
    probeTablebase: async (_fen, requestId) => ({
      requestId,
      status: "noTables" as const,
      pieces: 32,
      maxPieces: 0,
      rounded: false,
      moves: [],
    }),
    probeEngine: async () => ({ name: "Demo engine" }),
    setSettings: async (next) => (settings = structuredClone(next)),
    chooseNewDatabase: async () => null,
    confirmEditing: async () => false,
    searchGames: async (_db, _filter, requestId) => {
      hits = all().filter((n) => n % 12 === 1);
      return {
        requestId,
        hits: String(hits.length),
        scanned: "2400",
        truncated: false,
        cancelled: false,
      };
    },
    cancelSearch: async () => {},
    findPosition: async (_db, _fen, requestId) => {
      hits = all().filter((n) => n % 12 === 1);
      return {
        requestId,
        hits: String(hits.length),
        scanned: "2400",
        truncated: false,
        cancelled: false,
        indexed: false,
      };
    },
    searchPosition: async (_db, query, requestId) => {
      hits = all().filter((n) => n % 7 === 1);
      return {
        requestId,
        hits: String(hits.length),
        scanned: "2400",
        truncated: false,
        cancelled: false,
        indexed: false && query.exact,
      };
    },
    readRows: async (_db, start, limit, requestId) => ({
      requestId,
      entries: list()
        .slice(Number(start) - 1, Number(start) - 1 + limit)
        .map(demoEntry),
    }),
    findRow: async (_db, ordinal) => {
      const i = list().indexOf(Number(ordinal));
      return i < 0 ? null : String(i + 1);
    },
    sortList: async (_db, column, descending, onProgress) => {
      if (column) {
        onProgress({ stage: "reading", done: "2400", total: "2400" });
        const key = demoKey[column];
        order = all().sort((a, b) => {
          const [x, y] = [key(demoEntry(a)), key(demoEntry(b))];
          const c = x < y ? -1 : x > y ? 1 : a - b;
          return descending ? -c : c;
        });
      } else order = undefined;
      return {
        rows: String(list().length),
        cancelled: false,
        column: column ?? undefined,
        descending,
      };
    },
    cancelSort: async () => {},
    // Names from the demo games: the famous players and their events.
    nameList: async (_db, kind) => ({
      rows: String(demoNames(kind).length),
    }),
    cancelNameList: async () => {},
    copyRows: async (_db, ranges) => ({
      games: String(
        ranges.reduce((n, r) => n + Number(r.end) - Number(r.start), 0),
      ),
      truncated: false,
    }),
    pasteGames: async () => ({
      pasted: "0",
      skipped: "0",
      incomplete: "0",
      cancelled: false,
      entryCount: "2400",
      treeStale: false,
    }),
    cancelPaste: async () => {},
    deleteRows: async () => "0",
    confirmPaste: async () => false,
    readNames: async (_db, kind, start, limit, requestId) => ({
      requestId,
      rows: demoNames(kind).slice(Number(start) - 1, Number(start) - 1 + limit),
    }),
    findName: async (_db, kind, prefix) => {
      const i = demoNames(kind).findIndex(
        (n) => (n.name ?? "").toLowerCase() >= prefix.toLowerCase(),
      );
      return String(i < 0 ? demoNames(kind).length : i + 1);
    },
    crosstable: async (_db, tournamentId, layout) => {
      const event = demoNames("tournaments")[tournamentId]?.name;
      const names = [
        ...new Set(
          entries
            .filter((e) => e.event === event)
            .flatMap((e) => [e.white!, e.black!]),
        ),
      ];
      return {
        event,
        layout: layout ?? "roundRobin",
        rounds: 0,
        games: 0,
        unscored: 0,
        truncated: false,
        players: names.map((name, i) => ({
          id: i,
          name,
          points: 0,
          games: 0,
          wins: 0,
          blackGames: 0,
          blackWins: 0,
          sonnebornBerger: 0,
          buchholz: 0,
          cells: names.map(() => []),
        })),
      };
    },
    playerStatistics: async (_db, playerId, onProgress) => {
      onProgress({ done: String(demoCount), total: String(demoCount) });
      return demoStatistics(playerId);
    },
    cancelPlayerStatistics: async () => {},
    statisticsGames: async (_db, playerId, _group, requestId) => {
      const name = demoNames("players")[playerId]?.name;
      hits = all().filter((n) => {
        const e = demoEntry(n);
        return e.white === name || e.black === name;
      });
      return {
        requestId,
        hits: String(hits.length),
        scanned: String(hits.length),
        truncated: false,
        cancelled: false,
        indexed: false,
      };
    },
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
    clearSearch: async () => {
      hits = undefined;
    },
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
