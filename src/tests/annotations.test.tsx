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
