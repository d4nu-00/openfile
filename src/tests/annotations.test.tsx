import { render, screen } from "@testing-library/react";
import { it, expect } from "vitest";
import { NotationTree } from "../components/NotationTree";
import { Board } from "../components/Board";
import type { GameNode } from "../api/types";

const nodes: GameNode[] = [
  { id: 0, children: [1], comments: ["Game introduction"], nags: [] },
  {
    id: 1,
    parent: 0,
    uci: "e2e4",
    san: "e4",
    children: [2, 3],
    comments: [],
    nags: [1],
  },
  {
    id: 2,
    parent: 1,
    uci: "e7e5",
    san: "e5",
    children: [],
    comments: ["main"],
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
    preComments: ["Sharper is"],
  },
];

it("shows game-level text and text before a variation's move", () => {
  const view = render(
    <NotationTree nodes={nodes} selection={0} onSelect={() => {}} />,
  );
  expect(screen.getByText("Game introduction")).toBeInTheDocument();
  const text = view.container.querySelector(".variation-block")!.textContent;
  expect(text).toMatch(/Sharper is\s*1\.\.\.\s*c5/);
  expect(screen.getByText("!")).toBeInTheDocument();
});

it("draws coloured squares and arrows for the current move", () => {
  const view = render(
    <Board
      fen="rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1"
      flipped={false}
      legalUci={[]}
      marks={{
        squares: [{ color: "R", square: "f7" }],
        arrows: [{ color: "G", from: "g1", to: "f3" }],
      }}
    />,
  );
  expect(
    screen.getByRole("button", { name: /^f7 / }).getAttribute("data-mark"),
  ).toBe("red");
  const arrows = screen.getByRole("img", { name: "Board arrows" });
  expect(arrows.querySelectorAll("g.arrow.green")).toHaveLength(1);
  expect(view.container.querySelector("title")?.textContent).toBe("g1–f3");
});

it("draws squares with the right button and arrows with Alt+drag", async () => {
  const { fireEvent } = await import("@testing-library/react");
  const marks: unknown[] = [];
  const squares: string[] = [];
  render(
    <Board
      fen="rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
      flipped={false}
      legalUci={[]}
      onSquare={(s) => squares.push(s)}
      onMark={(m) => marks.push(m)}
    />,
  );
  const board = screen.getByRole("grid", { name: "Chess board" });
  board.getBoundingClientRect = () =>
    ({ left: 0, top: 0, width: 800, height: 800 }) as DOMRect;
  const e4 = screen.getByRole("button", { name: "e4 empty" });
  // Right click: a green square; with Shift, red.
  fireEvent.pointerDown(e4, { button: 2, clientX: 450, clientY: 450 });
  fireEvent.pointerUp(board);
  fireEvent.pointerDown(e4, { button: 2, shiftKey: true });
  fireEvent.pointerUp(board);
  // Alt+drag from g1 to f3 with Ctrl: a yellow arrow; the click that ends
  // it selects no square.
  const g1 = screen.getByRole("button", { name: "g1 White knight" });
  fireEvent.pointerDown(g1, { button: 0, altKey: true, ctrlKey: true });
  fireEvent.pointerMove(board, { clientX: 550, clientY: 550 });
  expect(screen.getByRole("img", { name: "Board arrows" })).toBeVisible();
  fireEvent.pointerUp(board);
  fireEvent.click(g1);
  expect(marks).toEqual([
    { from: "e4", to: "e4", color: "G" },
    { from: "e4", to: "e4", color: "R" },
    { from: "g1", to: "f3", color: "Y" },
  ]);
  expect(squares).toEqual([]);
  // A plain click still selects.
  fireEvent.click(g1);
  expect(squares).toEqual(["g1"]);
});
