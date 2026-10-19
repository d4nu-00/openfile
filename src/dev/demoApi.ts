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
  RepertoirePosition,
  WorkspaceSide,
  Line,
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
  const bookMoves = (moves: string[]) => {
    if (moves.length === 0)
      return [
        { uci: "e2e4", san: "e4" },
        { uci: "d2d4", san: "d4" },
        { uci: "g1f3", san: "Nf3" },
        { uci: "c2c4", san: "c4" },
        { uci: "b1c3", san: "Nc3" },
      ];
    if (moves.length === 1 && moves[0] === "e2e4")
      return [
        { uci: "e7e5", san: "e5" },
        { uci: "c7c5", san: "c5" },
        { uci: "e7e6", san: "e6" },
      ];
    let node = nodes[0];
    for (const uci of moves) {
      const next = node.children
        .map((id) => nodes[id])
        .find((n) => n.uci === uci);
      if (!next) return [];
      node = next;
    }
    return node.children.flatMap((id) => {
      const { uci, san } = nodes[id];
      return uci && san ? [{ uci, san }] : [];
    });
  };
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
  const repertoireEntries: RepertoirePosition[] = [];
  const demoSnapshot = (line: Line, side: WorkspaceSide) => {
    const legal = bookMoves(line.moves).map((move) => ({
      ...move,
      preferred: false,
      note: "",
      valid: true,
    }));
    const ownTurn = (line.moves.length % 2 === 0) === (side === "white");
    const key = `${side}|${line.moves.join(" ")}`;
    return {
      entries: repertoireEntries.filter((entry) =>
        entry.key.startsWith(`${side}|`),
      ),
      current: repertoireEntries.find((entry) => entry.key === key),
      ownTurn,
      legal,
    };
  };
  const api: Partial<WorkbenchApi> = {
    repertoireSnapshot: async (line, side) => demoSnapshot(line, side),
    repertoireEdit: async (line, side, uci, action, note) => {
      const key = `${side}|${line.moves.join(" ")}`;
      let entry = repertoireEntries.find((item) => item.key === key);
      const legal = bookMoves(line.moves);
      if (action === "add" && !entry) {
        entry = {
          key,
          fen: demoFens[line.moves.length] ?? demoFens[0],
          line,
          route: line.moves.join(" ") || "Start position",
          ownTurn: (line.moves.length % 2 === 0) === (side === "white"),
          moves: [],
          practice: { attempts: 0, correct: 0, needsRetry: false },
        };
        repertoireEntries.push(entry);
      }
      const move = entry?.moves.find((item) => item.uci === uci);
      if (entry && action === "add" && !move) {
        const found = legal.find((item) => item.uci === uci);
        if (found)
          entry.moves.push({
            ...found,
            preferred: entry.moves.length === 0,
            note: note ?? "",
            valid: true,
          });
      } else if (entry && action === "remove") {
        entry.moves = entry.moves.filter((item) => item.uci !== uci);
        if (entry.moves.length && !entry.moves.some((item) => item.preferred))
          entry.moves[0].preferred = true;
        if (!entry.moves.length)
          repertoireEntries.splice(repertoireEntries.indexOf(entry), 1);
      } else if (entry && action === "prefer") {
        entry.moves.forEach((item) => {
          item.preferred = item.uci === uci;
        });
      } else if (move && action === "note") move.note = note ?? "";
      return demoSnapshot(line, side);
    },
    repertoireGrade: async (line, side, uci) => {
      const entry = demoSnapshot(line, side).current;
      const correct = !!entry?.moves.some((m) => m.uci === uci);
      const playedSan =
        bookMoves(line.moves).find((m) => m.uci === uci)?.san ?? uci;
      const preferredSan = entry?.moves.find((m) => m.preferred)?.san;
      const practice = entry?.practice ?? {
        attempts: 0,
        correct: 0,
        needsRetry: false,
      };
      practice.attempts++;
      if (correct) practice.correct++;
      practice.needsRetry = !correct;
      return { correct, playedSan, preferredSan, practice: { ...practice } };
    },
    chooseDatabase: async () => "/demo/Famous Games.cbh",
    startupRequest: async () => null,
    startupShown: async () => {},
    getSettings: async () => structuredClone(settings),
    chooseTablebaseFolder: async () => null,
    blunderCheck: async () => ({
      cancelled: false,
      checked: 0,
      findings: [],
      evaluations: [],
      revision: "0",
    }),
    cancelBlunderCheck: async () => {},
    centipawnAnalysis: async (_doc, _params, onProgress) => {
      // Browser preview: one example finding from Morphy's Opera Game.
      onProgress({ done: 1, total: 1 });
      return {
        revision: String(revision),
        cancelled: false,
        positions: 1,
        bookPlies: 19,
        millisPerMove: 750,
        white: {
          moves: 0,
          averageLoss: null,
          accuracy: null,
          errorValue: null,
          categories: [0, 0, 0, 0, 0, 0, 0, 0],
        },
        black: {
          moves: 1,
          averageLoss: 168,
          accuracy: 33,
          errorValue: 0.84,
          categories: [0, 0, 0, 0, 0, 1, 0, 0],
        },
        items: [
          { node: 0, comment: "Centipawn Analysis after ply 19" },
          { node: 20, comment: "Mistake (−1.7)" },
        ],
        moves: [
          {
            node: 20,
            ply: 20,
            moveNumber: 10,
            san: "cxb5",
            white: false,
            loss: 168,
            category: "mistake" as const,
            bestUci: "e7b4",
            bestLine: ["Qb4+"],
          },
        ],
      };
    },
    cancelCentipawnAnalysis: async () => {},
    deepAnalysis: async (_doc, params) => ({
      cancelled: false,
      iterations: 0,
      positions: 0,
      limited: false,
      node: params.node,
      lines: [],
      revision: "0",
      best: [],
    }),
    stopDeepAnalysis: async () => {},
    openingReport: async () => {
      throw {
        code: "noTree",
        message: "The demo has no games to report on",
      };
    },
    classifyGame: async () => ({
      eco: "C42",
      name: "Petrov's Defense",
      ply: 4,
    }),
    setEcoCodes: async () => {
      throw { code: "readOnly", message: "The demo database is read-only" };
    },
    analysisJobs: async () => ({ jobs: [], running: false }),
    addAnalysisJobs: async () => {
      throw { code: "readOnly", message: "The demo database is read-only" };
    },
    addAnalysisJob: async () => {
      throw { code: "readOnly", message: "The demo database is read-only" };
    },
    editAnalysisJob: async () => ({ jobs: [], running: false }),
    clearAnalysisJobs: async () => ({ jobs: [], running: false }),
    runAnalysisJobs: async () => {
      throw { code: "noJobs", message: "There are no open positions" };
    },
    stopAnalysisJobs: async () => {},
    startScreen: async () => ({
      favorites: [
        {
          path: "/demo/test database.cbh",
          name: "test database",
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
    cloudStatus: async () => ({ files: [], bytes: "0" }),
    confirmCloudDownload: async () => false,
    downloadCloudFiles: async () => {},
    cancelCloudDownload: async () => {},
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
    chooseConvertTarget: async (name: string) => `/demo/${name} (2CBH).2cbh`,
    convertEstimate: async () => ({
      games: "2400",
      bytes: "4800000",
      free: "10000000000",
      fits: true,
    }),
    convertDatabase: async () => ({
      done: 0,
      written: 0,
      deleted: 0,
      incomplete: 0,
      texts: 0,
      chess960: 0,
      unreadable: 0,
      cancelled: true,
      path: null,
    }),
    cancelConvert: async () => {},
    checkIntegrity: async () => ({
      checked: 2400,
      games: 2398,
      texts: 2,
      deleted: 3,
      counts: {},
      problems: [],
      cancelled: false,
    }),
    findDoubleGames: async () => ({
      checked: "2400",
      marked: "0",
      keptBoth: "0",
      pairs: [],
      cancelled: false,
      treeStale: false,
    }),
    removeDeletedGames: async () => ({
      kept: "2400",
      removed: "0",
      bytesBefore: "0",
      bytesAfter: "0",
      cancelled: true,
      entryCount: "2400",
      moved: [],
      treeDeleted: false,
    }),
    cancelMaintenance: async () => {},
    markRows: async () => ({ changed: "0", marked: "0", cancelled: false }),
    setNamesOfRows: async () => ({
      changed: "0",
      marked: "0",
      cancelled: false,
    }),
    confirmRemoveDeleted: async () => false,
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
    prepareAgainst: async (_db, playerId, white, onProgress) => {
      onProgress({ stage: "lines", done: "1", total: "1" });
      const stats = demoStatistics(playerId);
      const name = stats.name;
      const mine = entries.filter(
        (e) =>
          e.kind === "game" &&
          !e.deleted &&
          (white ? e.white === name : e.black === name),
      );
      const score = white ? stats.white : stats.black;
      const lines: [string[], string, string, string][] = white
        ? [
            [
              ["e2e4", "e7e5", "g1f3"],
              "1.e4 e5 2.Nf3",
              "C40",
              "King's Knight Opening",
            ],
            [["d2d4", "d7d5", "c2c4"], "1.d4 d5 2.c4", "D06", "Queen's Gambit"],
          ]
        : [
            [["e2e4", "c7c5"], "1.e4 c5", "B20", "Sicilian Defense"],
            [
              ["d2d4", "g8f6", "c2c4", "e7e6"],
              "1.d4 Nf6 2.c4 e6",
              "E00",
              "Indian Defense",
            ],
          ];
      return {
        playerId,
        name,
        white,
        games: mine.length,
        deleted: 0,
        read: mine.length,
        unreadable: 0,
        truncated: false,
        score,
        recentRating: 2650,
        firstYear: stats.firstYear,
        lastYear: stats.lastYear,
        repertoire: lines.map(([moves, text, eco, opening], i) => ({
          moves,
          text,
          eco,
          name: opening,
          games: Math.max(1, Math.round(mine.length / (i + 2))),
          score: 60 - i * 10,
          lastYear: 2002 - i,
        })),
        openings: stats.openings
          .map((o) => ({ eco: o.eco, score: white ? o.white : o.black }))
          .filter((o) => o.score.games > 0),
        latest: mine
          .slice(-10)
          .reverse()
          .map((e) => ({
            ordinal: e.ordinal,
            year: Number(e.date?.slice(0, 4)) || undefined,
            opponent: white ? e.black : e.white,
            result: e.result,
            eco: e.eco,
            tournament: e.event,
          })),
        lastGame: mine.at(-1)?.ordinal,
      };
    },
    cancelPreparation: async () => {},
    chooseRepertoireFolder: async () => null,
    chooseRepertoireDatabase: async () => null,
    frequentPlayers: async () => [],
    generateRepertoire: async () => {
      throw { code: "unsupported", message: "Not available in the demo" };
    },
    repertoireReport: async () => {
      throw { code: "unsupported", message: "Not available in the demo" };
    },
    cancelRepertoire: async () => {},
    playerDossier: async (_db, playerId, onProgress) => {
      onProgress({ stage: "mates", done: "1", total: "1" });
      const stats = demoStatistics(playerId);
      const name = stats.name;
      const mine = entries.filter(
        (e) =>
          e.kind === "game" &&
          !e.deleted &&
          (e.white === name || e.black === name),
      );
      const colour = (white?: boolean) => {
        const list = mine.filter(
          (e) => white === undefined || (e.white === name) === white,
        );
        const count = (r: string) => list.filter((e) => e.result === r).length;
        const s =
          white === undefined ? undefined : white ? stats.white : stats.black;
        return {
          games: list.length,
          whiteWins: count("1-0"),
          draws: count("1/2-1/2"),
          blackWins: count("0-1"),
          score: s?.percent,
          scoreBand: s?.percent === undefined ? undefined : 4,
          opponentsAverage: s ? 2500 : undefined,
          performance: s ? 2550 : undefined,
          whiteWinMoves: 34,
          whiteWinBand: 2,
          drawMoves: 28,
          drawBand: 1,
          blackWinMoves: 41,
          blackWinBand: 3,
          drawPercent: 30,
          drawQuoteBand: 2,
          quickDrawPercent: 5,
        };
      };
      const rows = mine
        .slice(-5)
        .reverse()
        .map((e) => ({
          ordinal: e.ordinal,
          white: e.white === name,
          opponent: e.white === name ? e.black : e.white,
          year: Number(e.date?.slice(0, 4)) || undefined,
          result: e.result,
          moves: 30,
          eco: e.eco,
          tournament: e.event,
          medals: [] as string[],
        }));
      return {
        playerId,
        name,
        games: mine.length,
        deleted: 0,
        truncated: false,
        firstYear: stats.firstYear,
        lastYear: stats.lastYear,
        years: stats.years,
        all: colour(),
        white: colour(true),
        black: colour(false),
        ratingBands: [
          { from: 2450, games: 3, points: 2, percent: 67 },
          { from: 2500, games: 4, points: 2, percent: 50 },
        ],
        tournaments: stats.tournaments.slice(0, 10),
        opponents: stats.opponents.slice(0, 10),
        moreTournaments: 0,
        moreOpponents: 0,
        repertoireWhite: [
          {
            moves: ["e2e4", "e7e5"],
            text: "1.e4 e5",
            eco: "C20",
            name: "King's Pawn Game",
            games: 4,
            score: 62,
            lastYear: 2002,
          },
        ],
        repertoireBlack: [],
        medalGames: rows
          .slice(0, 1)
          .map((r) => ({ ...r, medals: ["Best game"] })),
        medalCount: 1,
        matesGiven: rows.slice(1, 2),
        matesGivenCount: 1,
        matesReceived: [],
        matesReceivedCount: 0,
        matesRead: mine.length,
        annotated: [],
        annotatedCount: 0,
      };
    },
    cancelDossier: async () => {},
    tournamentReport: async (db, tournamentId) => {
      const table = await api.crosstable!(db, tournamentId, "swiss");
      const rounds = Array.from({ length: table.rounds }, (_, i) => i + 1);
      const seen = new Set<number>();
      return {
        table,
        rounds: rounds.map((round) => ({
          round,
          date: `1999.05.${String(10 + round).padStart(2, "0")}`,
          games: table.players.flatMap((p, row) =>
            p.cells
              .flat()
              .filter(
                (g) => g.round === round && g.white && !seen.has(g.ordinal),
              )
              .map((g) => {
                seen.add(g.ordinal);
                return {
                  ordinal: g.ordinal,
                  white: row,
                  black: g.opponent,
                  result:
                    g.mark === "1" ? "1-0" : g.mark === "0" ? "0-1" : "1/2-1/2",
                  moves: 30 + round,
                };
              }),
          ),
          standings: table.players
            .slice(0, 5)
            .map((_, row) => ({ row, points: round / 2, games: round })),
        })),
        stats: {
          games: table.games,
          whiteWins: 0,
          draws: 0,
          blackWins: 0,
          forfeits: 0,
          averageMoves: 38,
        },
      };
    },
    selectionPlayers: async () => [
      { id: 0, name: demoNames("players")[0]?.name, games: 2 },
    ],
    errorReport: async (_db, params, onProgress) => {
      onProgress({ gamesDone: 1, games: 1, positions: 60 });
      return {
        cancelled: false,
        depth: params.slower ? 12 : 9,
        games: 1,
        skipped: 0,
        truncated: false,
        player: params.player === null ? null : demoNames("players")[0]?.name,
        moves: 30,
        averageLoss: 21.4,
        errorValue: 19.8,
        pliesPerGame: 62,
        blunderElo: 1990,
        rating: 2200,
        difference: -210,
        severity: [4, 2, 1, 0],
        distribution: [4, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        phases: { opening: { moves: 15, errors: 1, averageLoss: 12 } },
        opponents: { unrated: { moves: 30, errors: 3, averageLoss: 21.4 } },
        transitions: [
          [0, 0, 0, 0, 0],
          [0, 0, 1, 0, 0],
          [0, 0, 1, 1, 0],
          [0, 0, 0, 0, 0],
          [0, 0, 0, 0, 0],
        ],
        worst: [],
      };
    },
    cancelErrorReport: async () => {},
    showEntries: async (_db, ordinals, requestId) => {
      hits = ordinals.map(Number);
      return {
        requestId,
        hits: String(hits.length),
        scanned: String(hits.length),
        truncated: false,
        cancelled: false,
        indexed: false,
      };
    },
    closePreparation: async () => {},
    preparedTree: async (_db, line, requestId) => {
      const moves = line.moves.length === 0 ? ["e2e4", "d2d4"] : [];
      return {
        requestId,
        status: moves.length ? ("tree" as const) : ("notInTree" as const),
        games: moves.length ? 12 : 0,
        ended: 0,
        moves: moves.map((uci, i) => ({
          uci,
          san: uci === "e2e4" ? "e4" : "d4",
          games: 8 - i * 4,
          share: i ? 33.3 : 66.7,
          score: 60 - i * 10,
          draws: 30,
          averageElo: 2650,
          performance: 2700 - i * 50,
          firstYear: 1990,
          lastYear: 2002,
        })),
      };
    },
    preparationGames: async (_db, playerId, white, _line, requestId) => {
      const name = demoNames("players")[playerId]?.name;
      hits = all().filter((n) => {
        const e = demoEntry(n);
        return white ? e.white === name : e.black === name;
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
      const legal = bookMoves(line.moves).slice(0, 5);
      const counts = [1200, 640, 310, 95, 12];
      const total = counts.slice(0, legal.length).reduce((a, b) => a + b, 0);
      return {
        requestId,
        status: "tree" as const,
        games: total,
        ended: 0,
        moves: legal.map(({ uci, san }, i) => ({
          uci,
          san,
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
    filteredOpeningTree: async (_db, line, requestId) => {
      if (!hits)
        throw { code: "noSearch", message: "No current search result" };
      const legal = bookMoves(line.moves).slice(0, 3);
      const counts = [140, 45, 15];
      const total = hits.length;
      return {
        requestId,
        status: "filtered" as const,
        games: total,
        ended: 0,
        sampledHits: total,
        totalHits: total,
        moves: legal.map(({ uci, san }, i) => ({
          uci,
          san,
          games: counts[i],
          share: (counts[i] * 100) / total,
          score: 61 - i * 8,
          draws: 28 + i * 4,
          averageElo: 2490 - i * 55,
          performance: 2530 - i * 65,
          firstYear: 1950 + i * 5,
          lastYear: 2024 - i,
        })),
      };
    },
    cancelFilteredOpeningTree: async () => {},
    referenceSearch: async (_db, line, requestId) => ({
      requestId,
      hits: String(
        bookMoves(line.moves)
          .slice(0, 3)
          .reduce((n, _, i) => n + Math.max(1, 7 - i * 2), 0),
      ),
      truncated: false,
      cancelled: false,
      source: "tree" as const,
      indexed: false,
    }),
    cancelReference: async () => {},
    referenceStatistics: async (_db, line, requestId, filter) => {
      const legal = bookMoves(line.moves).slice(0, 3);
      const counts = legal.map((_, i) => Math.max(1, 7 - i * 2));
      const fullGames = counts.reduce((a, b) => a + b, 0);
      const eloOn = filter.eloMin !== undefined || filter.eloMax !== undefined;
      const yearOn =
        filter.yearMin !== undefined || filter.yearMax !== undefined;
      const filtered = counts.map((n) =>
        filter.goodGames || eloOn || yearOn ? Math.ceil(n / 2) : n,
      );
      const filteredGames = filtered.reduce((a, b) => a + b, 0);
      const afterElo = eloOn
        ? Math.min(
            fullGames,
            filteredGames + (yearOn ? 2 : filter.goodGames ? 1 : 0),
          )
        : fullGames;
      const afterYear = yearOn
        ? Math.min(afterElo, filteredGames + (filter.goodGames ? 1 : 0))
        : afterElo;
      return {
        requestId,
        fullGames,
        filteredGames,
        playerGames:
          filter.playerId === undefined ? 0 : Math.min(2, filteredGames),
        scannedGames: fullGames,
        afterElo,
        afterYear,
        missingElo: 1,
        missingYear: 1,
        complete: true,
        fullComplete: true,
        moves: legal.map(({ san }, i) => ({
          san,
          full: counts[i],
          filtered: filtered[i],
          player:
            filter.playerId === undefined
              ? 0
              : i === 0
                ? Math.min(2, filteredGames)
                : 0,
        })),
      };
    },
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
