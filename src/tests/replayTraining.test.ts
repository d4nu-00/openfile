import { describe, expect, it } from "vitest";
import type { GameView, PositionResponse } from "../api/types";
import {
  collectQuestionNodes,
  prepareQuestion,
  gradeAnswer,
  summary,
} from "../state/replayTraining";

const game: GameView = {
  tags: { White: "A", Black: "B" },
  startFen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
  nodes: [
    { id: 0, children: [1, 2], comments: [], nags: [] },
    {
      id: 1,
      parent: 0,
      uci: "e2e4",
      san: "e4",
      children: [],
      comments: [],
      nags: [],
      training: {
        kind: "moveInput",
        seconds: 30,
        score: 10,
        question: { texts: [{ language: 0, text: "Find a move" }] },
        wrong: { texts: [{ language: 0, text: "Try again" }] },
        answers: [
          {
            from: 33,
            to: 35,
            points: 10,
            feedback: { texts: [{ language: 0, text: "Good" }] },
          },
          { from: 1, to: 3, points: 0 },
        ],
      },
    },
    {
      id: 2,
      parent: 0,
      uci: "d2d4",
      san: "d4",
      children: [3],
      comments: [],
      nags: [],
    },
    {
      id: 3,
      parent: 2,
      uci: "d7d5",
      san: "d5",
      children: [],
      comments: [],
      nags: [],
      training: {
        kind: "multipleChoice",
        seconds: 0,
        score: 5,
        question: { texts: [{ language: 0, text: "Choose" }] },
        answers: [
          { points: 5, choice: { texts: [{ language: 0, text: "A" }] } },
        ],
      },
    },
  ],
};
const position: PositionResponse = {
  requestId: "r",
  documentId: "d",
  nodeId: 0,
  fen: game.startFen,
  turn: "white",
  legalUci: ["e2e4", "a2a4", "d2d4"],
};

describe("replay training model", () => {
  it("collects variation questions in tree order and keys by authored content", () => {
    expect(collectQuestionNodes(game)).toEqual([1, 3]);
    const first = prepareQuestion(game, 1, position, "Game 1");
    expect(first?.key).toContain("e2e4");
    expect(first?.key).not.toBe(
      prepareQuestion(
        {
          ...game,
          nodes: game.nodes.map((n) =>
            n.id === 1 ? { ...n, training: { ...n.training!, score: 8 } } : n,
          ),
        },
        1,
        position,
        "Game 1",
      )?.key,
    );
  });

  it("distinguishes authored wrong from ungraded legal moves", () => {
    const q = prepareQuestion(game, 1, position, "Game 1")!;
    expect(gradeAnswer(q, "e2e4")).toMatchObject({
      kind: "correct",
      points: 10,
      feedback: "Good",
    });
    expect(gradeAnswer(q, "a2a4")).toMatchObject({
      kind: "authoredWrong",
      points: 0,
      feedback: "Try again",
    });
    expect(gradeAnswer(q, "d2d4")).toMatchObject({
      kind: "ungraded",
      points: 0,
    });
    expect(gradeAnswer(q, "e2e5")).toBeNull();
  });

  it("does not credit an underpromotion as an unspecified queen answer", () => {
    const promotion = {
      ...game,
      nodes: game.nodes.map((n) =>
        n.id === 1
          ? {
              ...n,
              training: {
                ...n.training!,
                answers: [{ from: 6, to: 7, points: 10 }],
              },
            }
          : n,
      ),
    };
    const q = prepareQuestion(
      promotion,
      1,
      {
        ...position,
        fen: "8/P7/8/8/8/8/8/k6K w - - 0 1",
        legalUci: ["a7a8q", "a7a8n"],
      },
      "Promotion",
    )!;
    expect(gradeAnswer(q, "a7a8q")?.kind).toBe("correct");
    expect(gradeAnswer(q, "a7a8n")?.kind).toBe("ungraded");
  });

  it("reports solved and retry items separately", () => {
    expect(
      summary([{ key: "a" }, { key: "b" }] as never, {
        a: { attempts: 1, needsRetry: false },
        b: { attempts: 2, needsRetry: true },
      }),
    ).toEqual({ solved: 1, needsRetry: 1, unattempted: 0 });
  });

  it("skips a stale parent or a malformed annotation", () => {
    expect(
      prepareQuestion(game, 1, { ...position, nodeId: 2 }, "Stale"),
    ).toBeNull();
    const malformed = {
      ...game,
      nodes: game.nodes.map((n) =>
        n.id === 1
          ? {
              ...n,
              training: {
                ...n.training!,
                answers: [{ from: 99, to: 3, points: 10 }],
              },
            }
          : n,
      ),
    };
    expect(prepareQuestion(malformed, 1, position, "Malformed")).toBeNull();
  });
});
