import { it, expect } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import { renderWorkbench } from "./harness";
import { NotationTree } from "../components/NotationTree";
import {
  boardMarks,
  medalNames,
  packMoveTime,
  shortTime,
} from "../state/moveExtras";
import type { GameNode, GameView } from "../api/types";

const start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const nodes: GameNode[] = [
  { id: 0, children: [1], comments: [], nags: [] },
  {
    id: 1,
    parent: 0,
    uci: "g1f3",
    san: "Nf3",
    children: [2],
    comments: [],
    nags: [],
  },
  {
    id: 2,
    parent: 1,
    uci: "e7e5",
    san: "e5",
    children: [3],
    comments: [],
    nags: [],
  },
  {
    id: 3,
    parent: 2,
    uci: "f3e5",
    san: "Nxe5",
    children: [],
    comments: [],
    nags: [],
    extras: {
      medal: 8 | 512,
      moveTime: "0:00:44",
      diagram: true,
      pawnStructure: true,
      piecePath: "e5",
      evaluation: "+0.35/22",
    },
  },
];

it("reads and packs medals and move times", () => {
  expect(medalNames(8 | 512)).toEqual(["Novelty", "Sacrifice"]);
  expect(shortTime("0:00:44")).toBe("0:44");
  expect(shortTime("1:02:03")).toBe("1:02:03");
  expect(packMoveTime("1:05")).toBe(0x00010500);
  expect(packMoveTime("2:00:00")).toBe(0x02000000);
  expect(packMoveTime("1:75")).toBeUndefined();
});

it("shows a move's extras in the notation", () => {
  render(<NotationTree nodes={nodes} selection={3} onSelect={() => {}} />);
  expect(screen.getByText("Novelty, Sacrifice")).toBeVisible();
  expect(screen.getByText("0:44")).toBeVisible();
  expect(screen.getByText("+0.35/22")).toBeVisible();
  expect(screen.getByTitle("Diagram")).toBeVisible();
  expect(screen.getByText("Pawn structure")).toBeVisible();
  expect(screen.getByText("Piece path")).toBeVisible();
});

it("draws the pawn structure and the piece's path on the board", () => {
  const game: GameView = { tags: {}, startFen: start, nodes };
  const fen = "rnbqkbnr/pppp1ppp/8/4N3/8/8/PPPPPPPP/RNBQKB1R b KQkq - 0 2";
  const marks = boardMarks(game, 3, fen)!;
  expect(marks.squares).toHaveLength(15);
  expect(marks.arrows).toEqual([
    { color: "Y", from: "g1", to: "f3" },
    { color: "Y", from: "f3", to: "e5" },
  ]);
  // A move without extras keeps its own marks.
  expect(boardMarks(game, 1, fen)).toBe(nodes[1]);
});

it("sets a medal, a move time and a diagram as edits", async () => {
  const ui = renderWorkbench();
  await ui.user.click(ui.getByRole("button", { name: "e4" }));
  await ui.user.click(ui.getByRole("tab", { name: "Insert" }));
  await ui.user.click(ui.getByRole("button", { name: /Set Medal/ }));
  const medal = await screen.findByRole("dialog", { name: "Set Medal" });
  await ui.user.click(within(medal).getByRole("checkbox", { name: "Novelty" }));
  await ui.user.click(within(medal).getByRole("button", { name: "OK" }));
  await waitFor(() =>
    expect(ui.api.editDocument).toHaveBeenLastCalledWith(
      "doc-1",
      { kind: "setExtra", node: 1, extra: { kind: "medal", bits: 8 } },
      expect.any(String),
    ),
  );
  expect(await screen.findByText("Novelty")).toBeVisible();
  await ui.user.keyboard("{Control>}j{/Control}");
  const time = await screen.findByRole("dialog", { name: "Move Time" });
  await ui.user.type(
    within(time).getByRole("textbox", { name: "Time used" }),
    "1:05",
  );
  await ui.user.click(within(time).getByRole("button", { name: "OK" }));
  expect(await screen.findByText("1:05")).toBeVisible();
  await ui.user.click(ui.getByRole("button", { name: /Insert Diagram/ }));
  await waitFor(() => expect(screen.getByTitle("Diagram")).toBeVisible());
});
