import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWorkbench } from "./harness";

describe("tournament report", () => {
  it("lists the rounds of the chosen tournament and their games", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(screen.getByRole("tab", { name: "Tournaments" }));
    await ui.user.click(
      await screen.findByRole("button", { name: "Choose Open 2" }),
    );
    await ui.user.click(
      screen.getByRole("button", { name: "Tournament report" }),
    );
    expect(ui.api.tournamentReport).toHaveBeenCalledWith("db-1", 2);
    const dialog = await screen.findByRole("dialog", {
      name: "Tournament report",
    });
    const first = await within(dialog).findByRole("region", {
      name: "Round 1",
    });
    expect(within(first).getByText(/2020\.01\.10/)).toBeVisible();
    expect(within(first).getByText("Bravo, B")).toBeVisible();
    expect(
      within(first).getByText("After round 1: Alpha, A 1, Bravo, B 0"),
    ).toBeVisible();
    expect(within(dialog).getByText(/White scores 50\.0%/)).toBeVisible();

    // ChessBase's round range.
    await ui.user.clear(within(dialog).getByLabelText("First round"));
    await ui.user.type(within(dialog).getByLabelText("First round"), "2");
    expect(
      within(dialog).queryByRole("region", { name: "Round 1" }),
    ).toBeNull();
    const second = within(dialog).getByRole("region", { name: "Round 2" });
    // Unknown parts of a date are left out.
    expect(within(second).getByText(/Round 2 – 2020/)).toBeVisible();
    expect(within(second).queryByText(/\?\?/)).toBeNull();
    await ui.user.click(within(second).getByRole("button", { name: "Games" }));
    expect(ui.api.showEntries).toHaveBeenCalledWith(
      "db-1",
      ["8"],
      expect.any(String),
    );
    expect(await screen.findByText("Open 2: round 2: 1 games")).toBeVisible();
  });

  it("opens from the crosstable and loads a game", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(screen.getByRole("tab", { name: "Tournaments" }));
    await ui.user.click(
      await screen.findByRole("button", { name: "Choose Open 2" }),
    );
    await ui.user.click(screen.getByRole("button", { name: "Cross Table" }));
    const cross = await screen.findByRole("dialog", { name: "Cross table" });
    await ui.user.click(
      await within(cross).findByRole("button", { name: "Report" }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Tournament report",
    });
    const first = await within(dialog).findByRole("region", {
      name: "Round 1",
    });
    await ui.user.dblClick(within(first).getByText("Bravo, B"));
    await waitFor(() =>
      expect(ui.api.loadEntry).toHaveBeenLastCalledWith(
        "db-1",
        "7",
        expect.any(String),
      ),
    );
  });
});
