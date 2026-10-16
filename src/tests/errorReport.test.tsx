import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWorkbench } from "./harness";

describe("error report", () => {
  it("analyses the selected games for a player and shows the estimate", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(
      await screen.findByRole("button", { name: "Load entry 3" }),
    );
    await ui.user.keyboard("b");
    expect(ui.api.selectionPlayers).toHaveBeenCalledWith("db-1", [
      { start: "3", end: "4" },
    ]);
    const dialog = await screen.findByRole("dialog", { name: "Error Report" });
    // The player in most games is chosen.
    expect(within(dialog).getByLabelText("Moves of")).toHaveValue("3");
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Other engine…" }),
    );
    await ui.user.click(within(dialog).getByLabelText(/Slower/));
    await ui.user.click(within(dialog).getByRole("button", { name: "Start" }));
    expect(ui.api.errorReport).toHaveBeenCalledWith(
      "db-1",
      {
        engine: "/synthetic/mock-engine",
        options: {},
        slower: true,
        player: 3,
        ranges: [{ start: "3", end: "4" }],
      },
      expect.any(Function),
    );
    expect(await within(dialog).findByText("2190")).toBeVisible();
    expect(within(dialog).getByText(/-210 against Elo 2400/)).toBeVisible();
    expect(within(dialog).getByText(/at depth 12/)).toBeVisible();
    const severity = within(dialog).getByRole("region", { name: "Severity" });
    expect(within(severity).getByText("Blunders (100–299)")).toBeVisible();

    const worst = within(dialog).getByRole("region", { name: "Worst moves" });
    await ui.user.dblClick(within(worst).getByText("11. Qxb7"));
    await waitFor(() => expect(ui.api.loadEntry).toHaveBeenCalled());
    expect(screen.queryByRole("dialog", { name: "Error Report" })).toBeNull();
  });

  it("counts both sides when asked", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(
      await screen.findByRole("button", { name: "Load entry 3" }),
    );
    await ui.user.keyboard("b");
    const dialog = await screen.findByRole("dialog", { name: "Error Report" });
    await ui.user.selectOptions(
      within(dialog).getByLabelText("Moves of"),
      "both",
    );
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Other engine…" }),
    );
    await ui.user.click(within(dialog).getByRole("button", { name: "Start" }));
    expect(ui.api.errorReport).toHaveBeenCalledWith(
      "db-1",
      expect.objectContaining({ player: null, slower: false }),
      expect.any(Function),
    );
    expect(await within(dialog).findByText(/Both sides:/)).toBeVisible();
  });
});
