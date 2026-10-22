import { it, expect } from "vitest";
import { screen, within } from "@testing-library/react";
import { renderWorkbench } from "./harness";

it("analyses the selected games of an editable database and saves the CSV", async () => {
  const ui = renderWorkbench({
    clean: true,
    settings: {
      engines: [{ id: "sf", name: "Engine", path: "/e", options: {} }],
      defaultEngine: "sf",
    },
  });
  ui.api.chooseDatabase.mockResolvedValue("/synthetic/edit.2cbh");
  await ui.user.click(screen.getByRole("button", { name: "Open for Editing" }));
  await screen.findByRole("tab", { name: "Database edit" });
  await ui.user.click(
    await screen.findByRole("button", { name: "Load entry 1" }),
  );
  ui.api.centipawnList.mockImplementation(async (_db, _r, _p, onProgress) => {
    onProgress({ game: 1, games: 1, done: 3, total: 9, label: "A – B" });
    return {
      cancelled: false,
      error: null,
      games: 1,
      analysed: 1,
      replaced: 1,
      skippedCount: 0,
      skipped: [],
      millisPerMove: 200,
      players: [
        {
          name: "Player A",
          elo: 2500,
          perform: 2591,
          points: 1,
          games: [
            {
              opponent: "Player  B",
              opponentElo: 2400,
              halfPoints: 2,
              value: 34,
              weight: 4000,
            },
          ],
          weightedAverage: 34,
        },
      ],
      csv: "Player Name,Elo\n",
    };
  });
  await ui.user.click(
    screen.getByRole("button", { name: /^Centipawn Analysis/ }),
  );
  const dialog = await screen.findByRole("dialog", {
    name: "Centipawn Analysis of the selected games",
  });
  await ui.user.clear(
    within(dialog).getByRole("spinbutton", { name: "Seconds" }),
  );
  await ui.user.type(
    within(dialog).getByRole("spinbutton", { name: "Seconds" }),
    "0",
  );
  await ui.user.click(within(dialog).getByRole("button", { name: "Analyse" }));
  expect(ui.api.centipawnList).toHaveBeenCalledWith(
    "db-1",
    [{ start: "1", end: "2" }],
    { engine: "/e", options: {}, seconds: 0, everyMove: false },
    expect.any(Function),
  );
  const table = await within(dialog).findByRole("table", { name: "Players" });
  expect(within(table).getByText("Player A")).toBeVisible();
  expect(within(table).getByText("2591")).toBeVisible();
  expect(within(dialog).getByText("1 analysed, 1 replaced.")).toBeVisible();
  await ui.user.click(
    within(dialog).getByRole("button", { name: "Save CSV…" }),
  );
  expect(ui.api.chooseCsv).toHaveBeenCalledWith("edit-CentiPawnLoss.csv");
  expect(ui.api.saveCsv).toHaveBeenCalledWith(
    "/synthetic/stats.csv",
    "Player Name,Elo\n",
  );
});
