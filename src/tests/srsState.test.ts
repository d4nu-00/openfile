import { describe, expect, it } from "vitest";
import type { SrsStudyCard } from "../api/types";
import {
  arrowOf,
  defaultOptions,
  effectiveGrade,
  finished,
  formatSteps,
  giveHint,
  intervalLabel,
  isTraineePly,
  opponentPlays,
  parseSteps,
  startRun,
  tryMove,
  validateOptions,
} from "../state/srs";

const state = {
  kind: "new" as const,
  step: 0,
  due: 0,
  interval: 0,
  ease: 2.5,
  reps: 0,
  lapses: 0,
};

/** 1. e4 e5 2. Nf3 with Black to play after 1. e4; another line plays d5. */
function card(extra: Partial<SrsStudyCard> = {}): SrsStudyCard {
  return {
    card: {
      id: "c1",
      startFen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
      moves: [
        { uci: "e2e4", san: "e4", comment: null },
        { uci: "e7e5", san: "e5", comment: "Symmetry" },
      ],
      trainee: "black",
      white: "W",
      black: "B",
      event: "E",
      date: "2020.01.01",
      sourceLabel: "x.pgn · game 1",
      order: 0,
      suspended: false,
      removed: false,
    },
    learn: false,
    state,
    plies: [
      {
        fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        legalUci: [],
        otherMoves: [],
      },
      {
        fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
        legalUci: ["e7e5", "d7d5", "c7c5"],
        otherMoves: ["d7d5"],
      },
    ],
    endFen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2",
    previews: [],
    ...extra,
  };
}

describe("playing a card", () => {
  it("knows whose move each ply is", () => {
    const c = card();
    expect(isTraineePly(c, 0)).toBe(false);
    expect(isTraineePly(c, 1)).toBe(true);
  });

  it("moves on when the opponent has played and when the card's move is played", () => {
    const c = card();
    let run = startRun();
    expect(finished(c, run)).toBe(false);
    run = opponentPlays(run);
    expect(run.ply).toBe(1);
    run = tryMove(c, run, "e7e5");
    expect(run).toMatchObject({ ply: 2, mistakes: 0, note: undefined });
    expect(finished(c, run)).toBe(true);
  });

  it("counts a mistake, shows the arrow and keeps the position until the right move", () => {
    const c = card();
    let run = opponentPlays(startRun());
    run = tryMove(c, run, "c7c5");
    expect(run).toMatchObject({ ply: 1, mistakes: 1, note: "mistake" });
    expect(arrowOf(c, run)).toEqual({ from: "e7", to: "e5" });
    run = tryMove(c, run, "g8f6");
    expect(run.mistakes).toBe(2);
    run = tryMove(c, run, "e7e5");
    expect(run).toMatchObject({ ply: 2, mistakes: 2, note: undefined });
    expect(arrowOf(c, run)).toBeUndefined();
  });

  it("notes a move of another line without a mistake", () => {
    const c = card();
    const run = tryMove(c, opponentPlays(startRun()), "d7d5");
    expect(run).toMatchObject({ ply: 1, mistakes: 0, note: "other" });
    expect(arrowOf(c, run)).toBeUndefined();
  });

  it("counts a hint and shows the arrow", () => {
    const c = card();
    const run = giveHint(c, opponentPlays(startRun()));
    expect(run.hints).toBe(1);
    expect(arrowOf(c, run)).toEqual({ from: "e7", to: "e5" });
  });

  it("shows the arrow for every trainee move of a new card and never counts a mistake", () => {
    const c = card({ learn: true });
    let run = opponentPlays(startRun());
    expect(arrowOf(c, run)).toEqual({ from: "e7", to: "e5" });
    run = tryMove(c, run, "c7c5");
    expect(run).toMatchObject({ ply: 1, mistakes: 0, note: "learn" });
    expect(arrowOf(c, run)).toEqual({ from: "e7", to: "e5" });
    run = tryMove(c, run, "e7e5");
    expect(run.ply).toBe(2);
  });

  it("ignores a move when it is not the trainee's turn", () => {
    const c = card();
    const run = startRun();
    expect(tryMove(c, run, "e2e4")).toBe(run);
  });
});

describe("grades", () => {
  it("is Again after a mistake and Hard at most after a hint", () => {
    expect(effectiveGrade("easy", 1, 0)).toBe("again");
    expect(effectiveGrade("good", 0, 2)).toBe("hard");
    expect(effectiveGrade("easy", 0, 1)).toBe("hard");
    expect(effectiveGrade("again", 0, 1)).toBe("again");
    expect(effectiveGrade("good", 0, 0)).toBe("good");
  });
});

describe("intervals as shown under the grade buttons", () => {
  it("are minutes, hours, days, months and years", () => {
    const minute = 60_000;
    expect(intervalLabel(30_000)).toBe("<1m");
    expect(intervalLabel(minute)).toBe("1m");
    expect(intervalLabel(10 * minute)).toBe("10m");
    expect(intervalLabel(90 * minute)).toBe("1.5h");
    expect(intervalLabel(86_400_000)).toBe("1d");
    expect(intervalLabel(2.5 * 86_400_000)).toBe("2.5d");
    expect(intervalLabel(40 * 86_400_000)).toBe("1.3mo");
    expect(intervalLabel(800 * 86_400_000)).toBe("2.2y");
  });
});

describe("deck options", () => {
  it("reads steps in minutes", () => {
    expect(parseSteps("1, 10")).toEqual([1, 10]);
    expect(parseSteps("")).toEqual([]);
    expect(parseSteps("1 10 15")).toEqual([1, 10, 15]);
    expect(parseSteps("0.5")).toEqual([0.5]);
    expect(parseSteps("a")).toBeUndefined();
    expect(parseSteps("1,,2")).toEqual([1, 2]);
    expect(parseSteps("-5")).toBeUndefined();
    expect(formatSteps([1, 10])).toBe("1, 10");
  });

  it("has the defaults of the plan and accepts them", () => {
    expect(defaultOptions).toMatchObject({
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
    });
    expect(validateOptions(defaultOptions)).toBeUndefined();
  });

  it("refuses what the scheduler cannot work with", () => {
    const bad = (o: Partial<typeof defaultOptions>) =>
      validateOptions({ ...defaultOptions, ...o });
    expect(bad({ learningSteps: [] })).toMatch(/learning steps/i);
    expect(bad({ learningSteps: [0] })).toMatch(/learning steps/i);
    expect(bad({ relearningSteps: [-1] })).toMatch(/relearning steps/i);
    expect(bad({ graduatingInterval: 0 })).toMatch(/interval/i);
    expect(bad({ easyInterval: 0.5 })).toMatch(/interval/i);
    expect(bad({ graduatingInterval: 5, easyInterval: 4 })).toMatch(/easy/i);
    expect(bad({ maxInterval: 50000 })).toMatch(/maximum/i);
    expect(bad({ newPerDay: -1 })).toMatch(/per day/i);
    expect(bad({ reviewsPerDay: 1.5 })).toMatch(/per day/i);
    expect(bad({ relearningSteps: [] })).toBeUndefined();
  });
});
