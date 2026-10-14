import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWorkbench } from "./harness";

describe("player statistics", () => {
  it("scores the chosen player and lists a row's games", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(screen.getByRole("tab", { name: "Players" }));
    await ui.user.click(
      await screen.findByRole("button", { name: "Choose Player 3, A" }),
    );
    await ui.user.keyboard("s");
    expect(ui.api.playerStatistics).toHaveBeenCalledWith(
      "db-1",
      3,
      expect.any(Function),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Player statistic",
    });
    expect(
      await within(dialog).findByText("Statistics for Player 3, A"),
    ).toBeVisible();
    const colours = within(dialog).getByRole("table", {
      name: "Score by colour",
    });
    const total = within(colours).getByRole("rowheader", {
      name: "Total",
    }).parentElement!;
    expect(within(total).getByText("83%")).toBeVisible();
    expect(within(total).getByText("2600")).toBeVisible();

    // Sorting the opponents by name puts Alpha first.
    const opponents = within(dialog).getByRole("region", {
      name: "Opponents",
    });
    const names = () =>
      within(opponents)
        .getAllByRole("row")
        .slice(1)
        .map((r) => r.firstChild?.textContent);
    expect(names()).toEqual(["Bravo, B", "Alpha, C"]);
    await ui.user.click(within(opponents).getByRole("button", { name: "A-Z" }));
    expect(names()).toEqual(["Alpha, C", "Bravo, B"]);

    // The Table button shows the chosen tournament's crosstable.
    const events = within(dialog).getByRole("region", { name: "Tournaments" });
    await ui.user.click(within(events).getByText("Club"));
    await ui.user.click(within(events).getByRole("button", { name: "Table" }));
    expect(ui.api.crosstable).toHaveBeenCalledWith("db-1", 8, null);
    await ui.user.click(
      within(
        await screen.findByRole("dialog", { name: "Cross table" }),
      ).getByRole("button", { name: "Close" }),
    );

    await ui.user.dblClick(within(opponents).getByText("Bravo, B"));
    expect(ui.api.statisticsGames).toHaveBeenCalledWith(
      "db-1",
      3,
      { kind: "opponent", id: 5 },
      expect.any(String),
    );
    expect(
      await screen.findByText("Player 3, A: against Bravo, B: 2 games"),
    ).toBeVisible();
    expect(
      screen.queryByRole("dialog", { name: "Player statistic" }),
    ).toBeNull();
  });

  it("offers both players of the shown game", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(
      await screen.findByRole("button", { name: "Load entry 3" }),
    );
    await waitFor(() => expect(ui.api.loadEntry).toHaveBeenCalled());
    await ui.user.click(
      await screen.findByRole("button", { name: "Player statistic" }),
    );
    expect(ui.api.playerStatistics).toHaveBeenLastCalledWith(
      "db-1",
      11,
      expect.any(Function),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Player statistic",
    });
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Black: Beta" }),
    );
    expect(ui.api.playerStatistics).toHaveBeenLastCalledWith(
      "db-1",
      12,
      expect.any(Function),
    );
    expect(
      await within(dialog).findByText("Statistics for Player 12, A"),
    ).toBeVisible();
  });
});
