import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { WorkbenchApi } from "../api/client";
import type { GameView } from "../api/types";
import { ReplayTrainingSession } from "../components/ReplayTrainingSession";
import { prepareQuestion } from "../state/replayTraining";

const game: GameView = {
  tags: { White: "A", Black: "B" },
  startFen: "8/8/8/8/8/8/4P3/4K2k w - - 0 1",
  nodes: [
    { id: 0, children: [1], comments: [], nags: [] },
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
        seconds: 0,
        score: 10,
        question: { texts: [{ language: 0, text: "Find the pawn push" }] },
        help: [
          { texts: [{ language: 0, text: "Use the e-pawn" }] },
          { texts: [] },
        ],
        answers: [
          {
            from: 33,
            to: 35,
            points: 10,
            feedback: { texts: [{ language: 0, text: "Well spotted" }] },
          },
        ],
      },
    },
  ],
};

describe("replay training session", () => {
  it("records an expired unanswered question as needing another attempt", async () => {
    const timedGame: GameView = {
      ...game,
      nodes: game.nodes.map((node) =>
        node.id === 1
          ? { ...node, training: { ...node.training!, seconds: 1 } }
          : node,
      ),
    };
    const record = vi.fn().mockResolvedValue(undefined);
    const api = {
      replayTrainingProgress: vi.fn().mockResolvedValue({}),
      replayTrainingRecord: record,
      positionAt: vi.fn().mockResolvedValue({
        requestId: "q",
        documentId: "doc",
        nodeId: 0,
        fen: timedGame.startFen,
        turn: "white",
        legalUci: ["e2e4"],
      }),
    } as unknown as WorkbenchApi;
    render(
      <ReplayTrainingSession
        api={api}
        source={{
          kind: "current",
          game: timedGame,
          documentId: "doc",
          label: "A–B",
        }}
        onClose={() => {}}
      />,
    );
    expect(
      await screen.findByText("Time expired", {}, { timeout: 3000 }),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Next question" }));
    expect(screen.getByText(/1 need another attempt/)).toBeVisible();
    await waitFor(() => expect(record).toHaveBeenCalledTimes(1));
  });

  it("summarizes this run separately from saved progress", async () => {
    const position = {
      requestId: "q",
      documentId: "doc",
      nodeId: 0,
      fen: game.startFen,
      turn: "white" as const,
      legalUci: ["e2e4"],
    };
    const key = prepareQuestion(game, 1, position, "A–B")!.key;
    const api = {
      replayTrainingProgress: vi.fn().mockResolvedValue({
        [key]: { attempts: 1, needsRetry: false, bestPoints: 10 },
      }),
      positionAt: vi.fn().mockResolvedValue(position),
    } as unknown as WorkbenchApi;
    render(
      <ReplayTrainingSession
        api={api}
        source={{ kind: "current", game, documentId: "doc", label: "A–B" }}
        onClose={() => {}}
      />,
    );
    expect(await screen.findByText("Find the pawn push")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Next question" }));
    expect(
      screen.getByText(/0 solved · 0 need another attempt · 1 not attempted/),
    ).toBeVisible();
  });

  it("hides the saved move until the player answers, then offers a retry and summary", async () => {
    const record = vi.fn().mockResolvedValue(undefined);
    const api = {
      replayTrainingProgress: vi.fn().mockResolvedValue({}),
      replayTrainingRecord: record,
      positionAt: vi.fn().mockResolvedValue({
        requestId: "q",
        documentId: "doc",
        nodeId: 0,
        fen: game.startFen,
        turn: "white",
        legalUci: ["e2e4", "e2e3"],
      }),
    } as unknown as WorkbenchApi;
    render(
      <ReplayTrainingSession
        api={api}
        source={{ kind: "current", game, documentId: "doc", label: "A–B" }}
        onClose={() => {}}
      />,
    );
    expect(await screen.findByText("Find the pawn push")).toBeVisible();
    expect(screen.queryByText("Well spotted")).not.toBeInTheDocument();
    expect(screen.queryByText(/e4 is the answer/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /e2 white pawn/i }));
    fireEvent.click(screen.getByRole("button", { name: /e3 empty/i }));
    expect(
      await screen.findByText(
        "This legal move has no authored grade; no engine assessment was made.",
      ),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /e2 white pawn/i }));
    fireEvent.click(screen.getByRole("button", { name: /e4 empty/i }));
    expect(await screen.findByText("Well spotted")).toBeVisible();
    expect(screen.getAllByText(/Needs another attempt/).length).toBeGreaterThan(
      0,
    );
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(screen.queryByText("Well spotted")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /e2 white pawn/i }));
    fireEvent.click(screen.getByRole("button", { name: /e4 empty/i }));
    await waitFor(() => expect(record).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Next question" }));
    expect(screen.getByText(/1 solved/)).toBeVisible();
  });

  it("shows authored multiple choice feedback on a Black position", async () => {
    const blackGame: GameView = {
      ...game,
      startFen: "4k2K/4p3/8/8/8/8/8/8 b - - 0 1",
      nodes: [
        { ...game.nodes[0], children: [1] },
        {
          ...game.nodes[1],
          uci: "e7e5",
          training: {
            kind: "multipleChoice",
            seconds: 0,
            score: 4,
            question: { texts: [{ language: 0, text: "Choose Black's plan" }] },
            wrong: { texts: [{ language: 0, text: "That loses time." }] },
            answers: [
              { points: 0, choice: { texts: [{ language: 0, text: "Wait" }] } },
              {
                points: 4,
                choice: { texts: [{ language: 0, text: "Push the pawn" }] },
                feedback: { texts: [{ language: 0, text: "Correct plan" }] },
              },
            ],
          },
        },
      ],
    };
    const api = {
      replayTrainingProgress: vi.fn().mockResolvedValue({}),
      replayTrainingRecord: vi.fn().mockResolvedValue(undefined),
      positionAt: vi.fn().mockResolvedValue({
        requestId: "q",
        documentId: "doc",
        nodeId: 0,
        fen: blackGame.startFen,
        turn: "black",
        legalUci: ["e7e5"],
      }),
    } as unknown as WorkbenchApi;
    render(
      <ReplayTrainingSession
        api={api}
        source={{
          kind: "current",
          game: blackGame,
          documentId: "doc",
          label: "Black game",
        }}
        onClose={() => {}}
      />,
    );
    expect(await screen.findByText("Choose Black's plan")).toBeVisible();
    expect(screen.getByRole("grid", { name: "Chess board" })).toHaveAttribute(
      "data-flipped",
      "true",
    );
    fireEvent.click(screen.getByRole("button", { name: "Wait" }));
    expect(screen.getByText("That loses time.")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Push the pawn" }));
    expect(screen.getByText("Correct plan")).toBeVisible();
  });

  it("closes temporary documents after scanning selected rows", async () => {
    const closeDocument = vi.fn().mockResolvedValue(undefined);
    const api = {
      replayTrainingProgress: vi.fn().mockResolvedValue({}),
      replayTrainingRecord: vi.fn().mockResolvedValue(undefined),
      loadEntry: vi.fn().mockResolvedValue({ game, documentId: "temporary" }),
      closeDocument,
      positionAt: vi.fn().mockResolvedValue({
        requestId: "q",
        documentId: "temporary",
        nodeId: 0,
        fen: game.startFen,
        turn: "white",
        legalUci: ["e2e4"],
      }),
    } as unknown as WorkbenchApi;
    render(
      <ReplayTrainingSession
        api={api}
        source={{
          kind: "selection",
          databaseId: "db",
          ranges: [[4, 5]],
          entry: async () => ({
            ordinal: "12",
            kind: "game",
            deleted: false,
            diagnostics: [],
          }),
        }}
        onClose={() => {}}
      />,
    );
    expect(await screen.findByText("Find the pawn push")).toBeVisible();
    expect(closeDocument).toHaveBeenCalledWith("temporary");
  });

  it("cancels a long selected-game scan and closes an in-flight document", async () => {
    let finishLoad!: (value: { game: GameView; documentId: string }) => void;
    const loadEntry = vi.fn().mockReturnValue(
      new Promise((resolve) => {
        finishLoad = resolve;
      }),
    );
    const closeDocument = vi.fn().mockResolvedValue(undefined);
    const positionAt = vi.fn();
    const onClose = vi.fn();
    const api = {
      replayTrainingProgress: vi.fn().mockResolvedValue({}),
      loadEntry,
      closeDocument,
      positionAt,
    } as unknown as WorkbenchApi;
    const { unmount } = render(
      <ReplayTrainingSession
        api={api}
        source={{
          kind: "selection",
          databaseId: "db",
          ranges: [[0, 100]],
          entry: async () => ({
            ordinal: "12",
            kind: "game",
            deleted: false,
            diagnostics: [],
          }),
        }}
        onClose={onClose}
      />,
    );
    await waitFor(() => expect(loadEntry).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole("button", { name: "Cancel scan" }));
    expect(onClose).toHaveBeenCalledTimes(1);
    unmount();
    finishLoad({ game, documentId: "in-flight" });
    await waitFor(() =>
      expect(closeDocument).toHaveBeenCalledWith("in-flight"),
    );
    expect(positionAt).not.toHaveBeenCalled();
    expect(loadEntry).toHaveBeenCalledTimes(1);
  });
});
