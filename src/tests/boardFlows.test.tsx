import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { WorkbenchApi } from "../api/client";
import type { GameView } from "../api/types";
import { PositionSetup } from "../components/PositionSetup";
import { ReplayTrainingSession } from "../components/ReplayTrainingSession";
import { ReviewPractice } from "../components/ReviewPractice";
import { BoardPane } from "../views/BoardPane";
import { mockApi, renderWorkbench, selected } from "./harness";

function drag(board: HTMLElement, from: string, to: string, flipped = false) {
  board.getBoundingClientRect = () =>
    ({ left: 0, top: 0, width: 800, height: 800 }) as DOMRect;
  const at = (square: string) => ({
    clientX: (flipped ? "hgfedcba" : "abcdefgh").indexOf(square[0]) * 100 + 50,
    clientY: (flipped ? "12345678" : "87654321").indexOf(square[1]) * 100 + 50,
  });
  const source = within(board).getByRole("button", {
    name: new RegExp(`^${from} `),
  });
  fireEvent.pointerDown(source, {
    button: 0,
    pointerId: 8,
    pointerType: "touch",
    ...at(from),
  });
  fireEvent.pointerMove(board, {
    pointerId: 8,
    pointerType: "touch",
    ...at(to),
  });
  fireEvent.pointerUp(board, { pointerId: 8, pointerType: "touch", ...at(to) });
}

it("edits a game with a touch drag and offers promotion choice", async () => {
  const ui = renderWorkbench({
    position: {
      requestId: "initial",
      documentId: "doc-1",
      nodeId: 0,
      fen: "7k/P7/8/8/8/8/8/7K w - - 0 1",
      turn: "white",
      legalUci: ["a7a8q", "a7a8r", "a7a8b", "a7a8n"],
    },
  });
  drag(ui.getByRole("grid", { name: "Chess board" }), "a7", "a8");
  expect(ui.api.editDocument).not.toHaveBeenCalled();
  await ui.user.click(ui.getByRole("button", { name: "Knight" }));
  expect(ui.api.editDocument).toHaveBeenCalledWith(
    "doc-1",
    { kind: "appendMove", parent: 0, uci: "a7a8n" },
    "0",
  );
});

it("captures into promotion without shrinking the board, and can cancel or underpromote", async () => {
  const ui = renderWorkbench({
    position: {
      requestId: "initial",
      documentId: "doc-1",
      nodeId: 0,
      fen: "r6k/1P6/8/8/8/8/8/7K w - - 0 1",
      turn: "white",
      legalUci: ["b7a8q", "b7a8r", "b7a8b", "b7a8n"],
    },
  });
  const board = ui.getByRole("grid", { name: "Chess board" });
  drag(board, "b7", "a8");
  const chooser = ui.getByRole("dialog", { name: "Choose promotion" });
  expect(board.parentElement).toContainElement(chooser);
  const pieces = chooser.querySelector(".promotion-pieces");
  expect(pieces).toHaveClass("from-top");
  expect(pieces).toHaveStyle({ left: "6.25%" });
  expect(pieces?.querySelectorAll("button img")).toHaveLength(4);
  expect(within(chooser).getAllByRole("button")).toHaveLength(4);
  expect(within(chooser).getByRole("button", { name: "Queen" })).toHaveFocus();
  fireEvent.keyDown(chooser, { key: "Escape" });
  expect(
    ui.queryByRole("dialog", { name: "Choose promotion" }),
  ).not.toBeInTheDocument();
  expect(ui.api.editDocument).not.toHaveBeenCalled();
  drag(board, "b7", "a8");
  fireEvent.click(ui.getByRole("dialog", { name: "Choose promotion" }));
  expect(
    ui.queryByRole("dialog", { name: "Choose promotion" }),
  ).not.toBeInTheDocument();
  drag(board, "b7", "a8");
  await ui.user.click(ui.getByRole("button", { name: "Knight" }));
  expect(ui.api.editDocument).toHaveBeenCalledWith(
    "doc-1",
    { kind: "appendMove", parent: 0, uci: "b7a8n" },
    "0",
  );
});

it("anchors a black capture promotion to the right file after flipping", () => {
  const position = {
    requestId: "test",
    documentId: "doc-1",
    nodeId: 0,
    fen: "7k/8/8/8/8/8/1p6/R6K b - - 0 1",
    turn: "black" as const,
    legalUci: ["b2a1q", "b2a1r", "b2a1b", "b2a1n"],
  };
  const ui = render(
    <BoardPane
      title="Test"
      position={position}
      flipped={false}
      hasGame
      navigate={() => {}}
      promotion={position.legalUci}
      onPromote={() => {}}
    />,
  );
  expect(
    ui
      .getByRole("dialog", { name: "Choose promotion" })
      .querySelector(".promotion-pieces"),
  ).toHaveClass("from-bottom");
  ui.rerender(
    <BoardPane
      title="Test"
      position={position}
      flipped
      hasGame
      navigate={() => {}}
      promotion={position.legalUci}
      onPromote={() => {}}
    />,
  );
  const pieces = ui
    .getByRole("dialog", { name: "Choose promotion" })
    .querySelector(".promotion-pieces");
  expect(pieces).toHaveClass("from-top");
  expect(pieces).toHaveStyle({ left: "93.75%" });
});

it("enters Chess960 castling by dropping the king on its rook", () => {
  const ui = renderWorkbench({
    position: {
      requestId: "initial",
      documentId: "doc-1",
      nodeId: 0,
      fen: "1k6/8/8/8/8/8/8/RK5R w HA - 0 1",
      turn: "white",
      legalUci: ["b1h1", "b1a1"],
    },
  });
  drag(ui.getByRole("grid", { name: "Chess board" }), "b1", "h1");
  expect(ui.api.editDocument).toHaveBeenCalledWith(
    "doc-1",
    { kind: "appendMove", parent: 0, uci: "b1h1" },
    "0",
  );
});

it("relocates a piece in position setup without adding another one", () => {
  render(
    <PositionSetup
      boardFen="7k/8/8/8/8/8/4P3/7K w - - 0 1"
      onApply={() => {}}
      onClose={() => {}}
    />,
  );
  const dialog = screen.getByRole("dialog", { name: "Position setup" });
  drag(within(dialog).getByRole("grid", { name: "Chess board" }), "e2", "e4");
  expect(within(dialog).getByLabelText("FEN")).toHaveValue(
    "7k/8/8/8/4P3/8/8/7K w - - 0 1",
  );
});

it("grades a review practice move made by dragging", async () => {
  const api = mockApi();
  const onResult = vi.fn();
  const move = {
    node: 1,
    ply: 1,
    moveNumber: 1,
    san: "e4",
    white: true,
    loss: 120,
    category: "mistake" as const,
    bestUci: "d2d4",
    bestLine: ["d4"],
  };
  // Flush the asynchronous position load and the board's mount effects before
  // sending pointer events; a square can appear before those effects finish.
  await act(async () => {
    render(
      <ReviewPractice
        api={api}
        documentId="doc-1"
        game={selected.game!}
        moves={[move]}
        progress={{}}
        onResult={onResult}
      />,
    );
  });
  const practice = screen.getByRole("region", { name: "Learn from mistakes" });
  await within(practice).findByRole("button", { name: "d2 White pawn" });
  drag(within(practice).getByRole("grid", { name: "Chess board" }), "d2", "d4");
  expect(within(practice).getByText(/Correct: 1\. d4/)).toBeVisible();
  expect(onResult).toHaveBeenCalledWith(move, "solved");
});

it("answers a replay training question by dragging", async () => {
  const game: GameView = {
    startFen: "8/8/8/8/8/8/4P3/4K2k w - - 0 1",
    tags: { White: "A", Black: "B" },
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
          question: { texts: [{ language: 0, text: "Find e4" }] },
          answers: [
            {
              from: 33,
              to: 35,
              points: 10,
              feedback: { texts: [{ language: 0, text: "Correct move" }] },
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
      fen: game.startFen,
      turn: "white",
      legalUci: ["e2e4"],
    }),
  } as unknown as WorkbenchApi;
  render(
    <ReplayTrainingSession
      api={api}
      source={{ kind: "current", game, documentId: "doc", label: "A–B" }}
      onClose={() => {}}
    />,
  );
  await screen.findByText("Find e4");
  drag(screen.getByRole("grid", { name: "Chess board" }), "e2", "e4");
  expect(await screen.findByText("Correct move")).toBeVisible();
  await waitFor(() => expect(api.replayTrainingRecord).toHaveBeenCalled());
});
