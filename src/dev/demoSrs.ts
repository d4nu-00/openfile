// Demo backend of the Spaced Repetition decks for browser previews (dev builds
// only). It keeps one deck of a few mate puzzles in memory and schedules with
// fixed steps; the real scheduler is the backend's (docs/spaced-repetition.md).
import type { WorkbenchApi } from "../api/client";
import type {
  SrsCard,
  SrsCardState,
  SrsDeckSummary,
  SrsGrade,
  SrsOptions,
  SrsPreview,
  SrsQueueResponse,
  SrsStats,
  SrsStudyCard,
} from "../api/types";

const minute = 60_000;
const day = 86_400_000;

interface Puzzle {
  start: string;
  label: string;
  trainee: "white" | "black";
  moves: { uci: string; san: string }[];
  /** The position before each move and the trainee's legal moves there. */
  plies: { fen: string; legal: string[] }[];
  end: string;
}
const puzzles: Puzzle[] = [
  {
    start: "6k1/5ppp/8/8/8/8/5PPP/R5K1 w - - 0 1",
    label: "Mate in 1 · back rank",
    trainee: "white",
    moves: [{ uci: "a1a8", san: "Ra8#" }],
    plies: [
      {
        fen: "6k1/5ppp/8/8/8/8/5PPP/R5K1 w - - 0 1",
        legal:
          "g1h1 g1f1 a1a8 a1a7 a1a6 a1a5 a1a4 a1a3 a1a2 a1f1 a1e1 a1d1 a1c1 a1b1 h2h3 g2g3 f2f3 h2h4 g2g4 f2f4".split(
            " ",
          ),
      },
    ],
    end: "R5k1/5ppp/8/8/8/8/5PPP/6K1 b - - 1 1",
  },
  {
    start: "r5k1/5ppp/8/8/8/8/5PPP/6K1 b - - 0 1",
    label: "Mate in 1 · back rank, Black",
    trainee: "black",
    moves: [{ uci: "a8a1", san: "Ra1#" }],
    plies: [
      {
        fen: "r5k1/5ppp/8/8/8/8/5PPP/6K1 b - - 0 1",
        legal:
          "g8h8 g8f8 a8f8 a8e8 a8d8 a8c8 a8b8 a8a7 a8a6 a8a5 a8a4 a8a3 a8a2 a8a1 h7h6 g7g6 f7f6 h7h5 g7g5 f7f5".split(
            " ",
          ),
      },
    ],
    end: "6k1/5ppp/8/8/8/8/5PPP/r5K1 w - - 1 2",
  },
  {
    start: "8/8/8/8/7R/4K3/8/6k1 w - - 0 1",
    label: "Mate in 2 · rook and king",
    trainee: "white",
    moves: [
      { uci: "e3f3", san: "Kf3" },
      { uci: "g1f1", san: "Kf1" },
      { uci: "h4h1", san: "Rh1#" },
    ],
    plies: [
      {
        fen: "8/8/8/8/7R/4K3/8/6k1 w - - 0 1",
        legal:
          "h4h8 h4h7 h4h6 h4h5 h4g4 h4f4 h4e4 h4d4 h4c4 h4b4 h4a4 h4h3 h4h2 h4h1 e3f4 e3e4 e3d4 e3f3 e3d3 e3e2 e3d2".split(
            " ",
          ),
      },
      { fen: "8/8/8/8/7R/5K2/8/6k1 b - - 1 1", legal: [] },
      {
        fen: "8/8/8/8/7R/5K2/8/5k2 w - - 2 2",
        legal:
          "h4h8 h4h7 h4h6 h4h5 h4g4 h4f4 h4e4 h4d4 h4c4 h4b4 h4a4 h4h3 h4h2 h4h1 f3g4 f3f4 f3e4 f3g3 f3e3".split(
            " ",
          ),
      },
    ],
    end: "8/8/8/8/8/5K2/8/5k1R b - - 3 2",
  },
  {
    start:
      "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4",
    label: "Mate in 1 · Scholar's mate",
    trainee: "white",
    moves: [{ uci: "h5f7", san: "Qxf7#" }],
    plies: [
      {
        fen: "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4",
        legal:
          "h5h7 h5f7 h5h6 h5g6 h5g5 h5f5 h5e5 h5h4 h5g4 h5h3 h5f3 h5e2 h5d1 c4f7 c4e6 c4a6 c4d5 c4b5 c4d3 c4b3 c4e2 c4f1 g1h3 g1f3 g1e2 e1e2 e1f1 e1d1 b1c3 b1a3 h2h3 g2g3 f2f3 d2d3 c2c3 b2b3 a2a3 h2h4 g2g4 f2f4 d2d4 b2b4 a2a4".split(
            " ",
          ),
      },
    ],
    end: "r1bqkb1r/pppp1Qpp/2n2n2/4p3/2B1P3/8/PPPP1PPP/RNB1K1NR b KQkq - 0 4",
  },
];

const cards: SrsCard[] = puzzles.map((p, i) => ({
  id: `demo-card-${i + 1}`,
  startFen: p.start,
  moves: p.moves.map((m) => ({ ...m, comment: null })),
  trainee: p.trainee,
  white: "Demo",
  black: "Puzzle",
  event: p.label,
  date: "",
  sourceLabel: `Demo mates · game ${i + 1}`,
  order: i,
  suspended: false,
  removed: false,
}));

/** What a grade does to a card in the demo: fixed steps, not the real rules. */
function demoAnswer(
  state: SrsCardState,
  grade: SrsGrade,
  now: number,
): SrsCardState {
  const next = { ...state };
  if (grade === "again") {
    Object.assign(next, { kind: "learning", step: 0, due: now + minute });
  } else if (grade === "easy") {
    Object.assign(next, { kind: "review", interval: 4, due: now + 4 * day });
  } else if (state.kind === "review") {
    const interval = Math.max(state.interval + 1, state.interval * 2.5);
    Object.assign(next, { interval, due: now + interval * day });
  } else if (state.kind === "new" || grade === "hard") {
    Object.assign(next, {
      kind: "learning",
      step: 0,
      due: now + (grade === "hard" ? 6 : 10) * minute,
    });
    if (state.kind === "new" && grade === "good") next.step = 1;
  } else {
    Object.assign(next, { kind: "review", interval: 1, due: now + day });
  }
  next.reps = state.reps + 1;
  return next;
}

export function demoSrs(): Partial<WorkbenchApi> {
  const states = new Map<string, SrsCardState>();
  let deck: { id: string; name: string; options?: SrsOptions } | undefined;
  const stateOf = (id: string): SrsCardState =>
    states.get(id) ?? {
      kind: "new",
      step: 0,
      due: 0,
      interval: 0,
      ease: 2.5,
      reps: 0,
      lapses: 0,
    };
  const stats = (): SrsStats => {
    const live = cards.filter((c) => !c.suspended);
    const kinds = live.map((c) => stateOf(c.id).kind);
    return {
      total: cards.length,
      new: kinds.filter((k) => k === "new").length,
      learning: kinds.filter((k) => k === "learning" || k === "relearning")
        .length,
      dueToday: kinds.filter((k) => k === "review").length,
      suspended: cards.length - live.length,
      mature: 0,
      reviewsToday: 0,
      retention: null,
      forecast: [0, 0, 0, 0, 0, 0, 0],
    };
  };
  const summary = (): SrsDeckSummary => ({
    id: deck!.id,
    name: deck!.name,
    source: "Demo mates.pgn",
    sourceKind: "pgn",
    cards: cards.length,
    stats: stats(),
  });
  const study = (card: SrsCard, now: number): SrsStudyCard => {
    const puzzle = puzzles[card.order];
    const state = stateOf(card.id);
    const previews: SrsPreview[] = (
      ["again", "hard", "good", "easy"] as SrsGrade[]
    ).map((grade) => {
      const after = demoAnswer(state, grade, now);
      return {
        grade,
        dueInMs: after.due - now,
        intervalDays: after.interval,
        kind: after.kind,
      };
    });
    return {
      card,
      learn: state.kind === "new",
      state,
      plies: puzzle.plies.map((p) => ({
        fen: p.fen,
        legalUci: p.legal,
        otherMoves: [],
      })),
      endFen: puzzle.end,
      previews,
    };
  };
  const queue = (now: number): SrsQueueResponse => {
    const live = cards.filter((c) => !c.suspended);
    const due = (c: SrsCard) => {
      const s = stateOf(c.id);
      return s.kind === "new" || s.due <= now + 20 * minute;
    };
    const ready = live
      .filter(due)
      .sort(
        (a, b) => stateOf(a.id).due - stateOf(b.id).due || a.order - b.order,
      );
    const kind = (k: string[]) =>
      ready.filter((c) => k.includes(stateOf(c.id).kind)).length;
    return {
      queue: {
        cards: ready.map((c) => c.id),
        newLeft: 20,
        reviewsLeft: 200,
        learningDue: kind(["learning", "relearning"]),
        reviewDue: kind(["review"]),
        newDue: kind(["new"]),
        nextLearningDue: null,
        ahead: false,
      },
      next: ready[0] ? study(ready[0], now) : null,
      stats: stats(),
    };
  };
  const make = async (_a: unknown, name: string, ..._rest: unknown[]) => {
    deck = { id: "demo-deck", name };
    states.clear();
    return {
      deck: summary(),
      skipped: { games: 0, cardsOverLimit: 0, gamesNotRead: 0 },
    };
  };
  return {
    srsDecks: async () => ({ decks: deck ? [summary()] : [], unreadable: [] }),
    srsCreateFromPgn: (path, name) => make(path, name),
    srsCreateFromRows: (_db, _ranges, name) => make(_db, name),
    srsUpdateFromPgn: async () => ({
      deck: summary(),
      skipped: { games: 0, cardsOverLimit: 0, gamesNotRead: 0 },
      added: 0,
      kept: cards.length,
      suspended: 0,
      restored: 0,
    }),
    srsUpdateFromRows: async () => ({
      deck: summary(),
      skipped: { games: 0, cardsOverLimit: 0, gamesNotRead: 0 },
      added: 0,
      kept: cards.length,
      suspended: 0,
      restored: 0,
    }),
    srsDeck: async () => ({
      id: deck!.id,
      name: deck!.name,
      source: "Demo mates.pgn",
      sourceKind: "pgn",
      created: 0,
      options: deck!.options ?? {
        learningSteps: [1, 10],
        relearningSteps: [10],
        graduatingInterval: 1,
        easyInterval: 4,
        newPerDay: 20,
        reviewsPerDay: 200,
        maxInterval: 36500,
        newOrder: "source",
        includeVariations: true,
        trainee: "sideToMove",
      },
      cards: [],
    }),
    srsSetOptions: async (_id, options) => {
      if (deck) deck.options = options;
    },
    srsRename: async (_id, name) => {
      if (deck) deck.name = name;
    },
    srsDelete: async () => {
      deck = undefined;
      states.clear();
    },
    srsQueue: async (_id, now) => queue(now),
    srsAnswer: async (_id, cardId, grade, _mistakes, _hints, _ms, now) => {
      const state = demoAnswer(stateOf(cardId), grade, now);
      states.set(cardId, state);
      return { state, ...queue(now) };
    },
    srsSuspend: async (_id, cardId, suspended) => {
      const card = cards.find((c) => c.id === cardId);
      if (card) card.suspended = suspended;
    },
    choosePgn: async () => "/demo/Demo mates.pgn",
    confirmDeleteDeck: async () => true,
  };
}
