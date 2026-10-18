import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWorkbench } from "./harness";

describe("generate repertoire", () => {
  it("generates from the chosen players and reports on the White repertoire", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(ui.getByRole("tab", { name: "Report" }));
    await ui.user.click(
      ui.getByRole("button", { name: "Generate Repertoire" }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Generate Repertoire",
    });
    expect(ui.api.frequentPlayers).toHaveBeenCalledWith("db-1");
    // The most frequent player is ticked.
    const alpha = await within(dialog).findByRole("checkbox", {
      name: /Alpha, A/,
    });
    expect(alpha).toBeChecked();
    const generate = within(dialog).getByRole("button", { name: "Generate" });
    expect(generate).toBeDisabled();
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Choose…" }),
    );
    expect(within(dialog).getByLabelText("Folder")).toHaveValue(
      "/synthetic/rep",
    );
    await ui.user.click(generate);
    expect(ui.api.generateRepertoire).toHaveBeenCalledWith(
      "db-1",
      [7],
      "/synthetic/rep",
      expect.any(Function),
    );
    const result = await within(dialog).findByRole("table", {
      name: "Result",
    });
    expect(within(result).getByText("Repertoire White")).toBeVisible();
    expect(within(result).getByText("not written")).toBeVisible();
    await ui.user.click(within(dialog).getByRole("button", { name: "Close" }));

    // The report starts from the White database just written.
    await ui.user.click(
      ui.getByRole("button", { name: "Repertoire Report White" }),
    );
    const report = await screen.findByRole("dialog", {
      name: "Repertoire Report White",
    });
    expect(within(report).getByLabelText("Repertoire database")).toHaveValue(
      "/synthetic/rep/Repertoire White.2cbh",
    );
    await ui.user.click(within(report).getByRole("button", { name: "Report" }));
    expect(ui.api.repertoireReport).toHaveBeenCalledWith(
      "db-1",
      "/synthetic/rep/Repertoire White.2cbh",
      expect.any(Function),
    );
    expect(
      await within(report).findByText(/2 of 10 games reach one/),
    ).toBeVisible();
    expect(
      within(report).getByRole("heading", {
        name: "B90 Sicilian Defense (Najdorf Variation, English Attack)",
      }),
    ).toBeVisible();
    expect(within(report).getByText(/Delta, D 2700/)).toBeVisible();
    await ui.user.dblClick(within(report).getByText(/6\.Be3/));
    expect(ui.api.showEntries).toHaveBeenCalledWith(
      "db-1",
      ["2", "1"],
      expect.any(String),
    );
    expect(
      screen.queryByRole("dialog", { name: /Repertoire Report/ }),
    ).toBeNull();
  });
});
