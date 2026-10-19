import { expect, it, vi } from "vitest";
import { render, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockApi, selected } from "./harness";
import { ReviewPractice } from "../components/ReviewPractice";
import {
  practiceKey,
  practiceItemKey,
  readPractice,
  recordPractice,
  writePractice,
} from "../state/reviewPractice";

it("ties practice to the exact game line and suggested move", () => {
  const game = structuredClone(selected.game!);
  const key = practiceKey(game);
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
  const progress = recordPractice({}, move, "needsPractice");
  writePractice(key, progress);
  expect(readPractice(key)[practiceItemKey(move)]).toBe("needsPractice");
  expect(
    readPractice(key)[practiceItemKey({ ...move, bestUci: "g1f3" })],
  ).toBeUndefined();
  game.nodes[1].uci = "d2d4";
  expect(readPractice(practiceKey(game))).toEqual({});
  localStorage.removeItem(key);
});

it("moves through critical positions and loads the board before each move", async () => {
  const api = mockApi();
  const onResult = vi.fn();
  const moves = [
    {
      node: 1,
      ply: 1,
      moveNumber: 1,
      san: "e4",
      white: true,
      loss: 120,
      category: "mistake" as const,
      bestUci: "d2d4",
      bestLine: ["d4"],
    },
    {
      node: 2,
      ply: 2,
      moveNumber: 1,
      san: "e5",
      white: false,
      loss: 110,
      category: "mistake" as const,
      bestUci: "c7c5",
      bestLine: ["c5"],
    },
  ];
  const ui = render(
    <ReviewPractice
      api={api}
      documentId="doc-1"
      game={selected.game!}
      moves={moves}
      progress={{}}
      onResult={onResult}
    />,
  );
  const user = userEvent.setup();
  const practice = within(
    ui.getByRole("region", { name: "Learn from mistakes" }),
  );
  expect(await practice.findByText(/White to move/)).toBeVisible();
  await user.click(practice.getByRole("button", { name: "Next" }));
  expect(practice.getByText(/Black to move/)).toBeVisible();
  expect(api.positionAt).toHaveBeenCalledWith("doc-1", 1, expect.any(String));
  await user.click(practice.getByRole("button", { name: "c7 Black pawn" }));
  await user.click(practice.getByRole("button", { name: "c5 empty" }));
  expect(practice.getByText(/Correct: 1\.\.\. c5/)).toBeVisible();
  expect(onResult).toHaveBeenCalledWith(moves[1], "solved");
  await user.click(practice.getByRole("button", { name: "Previous" }));
  expect(practice.getByText(/White to move/)).toBeVisible();
});
