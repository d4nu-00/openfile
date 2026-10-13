import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWorkbench } from "./harness";

describe("players and tournaments", () => {
  it("lists players, finds a name and shows the player's games", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(screen.getByRole("tab", { name: "Players" }));
    expect(ui.api.nameList).toHaveBeenCalledWith(
      "db-1",
      "players",
      expect.any(Function),
    );
    expect(await screen.findByText("250 players")).toBeVisible();
    expect(
      await screen.findByRole("button", { name: "Choose Player 1, A" }),
    ).toBeVisible();
    await ui.user.type(
      screen.getByLabelText("Find player (Last, First)"),
      "Player 120",
    );
    await waitFor(() =>
      expect(ui.api.findName).toHaveBeenCalledWith(
        "db-1",
        "players",
        "Player 120",
      ),
    );
    const row = await screen.findByRole("button", {
      name: "Choose Player 120, A",
    });
    await waitFor(() =>
      expect(row.closest("tr")).toHaveAttribute("aria-selected", "true"),
    );
    await ui.user.dblClick(row);
    expect(ui.api.searchGames).toHaveBeenCalledWith(
      "db-1",
      expect.objectContaining({ playerId: 120 }),
      expect.any(String),
    );
    expect(
      await screen.findByText("Games of Player 120, A: 2 games"),
    ).toBeVisible();
    expect(screen.getByRole("tab", { name: "Games" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("shows a tournament's crosstable and opens a game from it", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(screen.getByRole("tab", { name: "Tournaments" }));
    await ui.user.click(
      await screen.findByRole("button", { name: "Choose Open 2" }),
    );
    await ui.user.click(screen.getByRole("button", { name: "Cross Table" }));
    expect(ui.api.crosstable).toHaveBeenCalledWith("db-1", 2, null);
    const dialog = await screen.findByRole("dialog", { name: "Cross table" });
    expect(within(dialog).getByText("Open 2")).toBeVisible();
    expect(within(dialog).getByText(/Category 6/)).toBeVisible();
    expect(within(dialog).getByText("Alpha, A")).toBeVisible();
    await ui.user.click(within(dialog).getByRole("button", { name: "Swiss" }));
    expect(ui.api.crosstable).toHaveBeenLastCalledWith("db-1", 2, "swiss");
    expect(await within(dialog).findByText("2w1")).toBeVisible();
    await ui.user.click(within(dialog).getByText("2w1"));
    await waitFor(() =>
      expect(ui.api.loadEntry).toHaveBeenLastCalledWith(
        "db-1",
        "7",
        expect.any(String),
      ),
    );
    expect(screen.queryByRole("dialog", { name: "Cross table" })).toBeNull();
  });
});
