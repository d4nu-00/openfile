import { it, expect } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { renderWorkbench } from "./harness";

it("finds the novelty, lists the games before it and selects it", async () => {
  const ui = renderWorkbench();
  ui.api.findNovelty.mockResolvedValue({
    novelty: { ply: 2, node: 2 },
    knownPly: 1,
    total: 3,
    games: [
      {
        ordinal: "5",
        kind: "game",
        deleted: false,
        white: "Player A",
        black: "Player B",
        result: "1-0",
        date: "2020.01.01",
        event: "Open",
      },
    ],
    queries: 3,
    notSearched: false,
    cancelled: false,
  });
  await ui.user.keyboard("{Shift>}{F6}{/Shift}");
  const dialog = await screen.findByRole("dialog", { name: "Find Novelty" });
  expect(ui.api.findNovelty).toHaveBeenCalledWith("doc-1", "db-1");
  expect(await within(dialog).findByText("1...e5")).toBeVisible();
  expect(within(dialog).getByText(/occurs in 3 other games/)).toBeVisible();
  const table = within(dialog).getByRole("table", {
    name: "Games before the novelty",
  });
  expect(within(table).getByText("Player A")).toBeVisible();
  expect(within(dialog).getByText("The first 1 of 3 games.")).toBeVisible();
  // The novelty is selected on the board.
  await waitFor(() =>
    expect(ui.api.positionAt.mock.calls.at(-1)?.slice(0, 2)).toEqual([
      "doc-1",
      2,
    ]),
  );
  await ui.user.click(within(dialog).getByRole("button", { name: "Close" }));
  expect(screen.queryByRole("dialog", { name: "Find Novelty" })).toBeNull();
});

it("says when the database cannot be searched for every move", async () => {
  const ui = renderWorkbench();
  ui.api.findNovelty.mockResolvedValue({
    novelty: null,
    knownPly: 0,
    total: 0,
    games: [],
    queries: 1,
    notSearched: true,
    cancelled: false,
  });
  await ui.user.keyboard("{Shift>}{F6}{/Shift}");
  const dialog = await screen.findByRole("dialog", { name: "Find Novelty" });
  expect(await within(dialog).findByRole("alert")).toHaveTextContent(
    "no opening tree or search booster",
  );
});
