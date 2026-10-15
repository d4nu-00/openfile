import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { renderWorkbench } from "./harness";

describe("opening classification", () => {
  it("classifies the shown game with Ctrl+Alt+C and sets its ECO", async () => {
    const ui = renderWorkbench();
    await ui.user.keyboard("{Control>}{Alt>}c{/Alt}{/Control}");
    const dialog = await ui.findByRole("dialog", { name: "Classify Opening" });
    expect(ui.api.classifyGame).toHaveBeenCalledWith("doc-1");
    expect(within(dialog).getByText("C20")).toBeVisible();
    expect(within(dialog).getByText("King's Pawn Game")).toBeVisible();
    expect(
      within(dialog).getByText("Reached after move 1 (Black)."),
    ).toBeVisible();
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Set ECO C20" }),
    );
    await waitFor(() =>
      expect(ui.api.editDocument).toHaveBeenCalledWith(
        "doc-1",
        {
          kind: "setTags",
          tags: expect.objectContaining({ ECO: "C20" }),
        },
        "0",
      ),
    );
    expect(ui.queryByRole("dialog", { name: "Classify Opening" })).toBeNull();
  });

  it("sets the ECO codes of an editable database", async () => {
    const ui = renderWorkbench({ clean: true });
    ui.api.chooseDatabase.mockResolvedValue("/synthetic/edit.2cbh");
    await ui.user.click(
      screen.getByRole("button", { name: "Open for Editing" }),
    );
    await screen.findByRole("tab", { name: "Database edit" });
    await ui.user.click(screen.getByRole("tab", { name: "Maintenance" }));
    await ui.user.click(screen.getByRole("button", { name: "Set ECO Codes" }));
    const options = await screen.findByRole("dialog", {
      name: "Set ECO Codes",
    });
    const only = within(options).getByRole("checkbox", {
      name: "Only games without an ECO code",
    });
    expect(only).toBeChecked();
    await ui.user.click(within(options).getByRole("button", { name: "Start" }));
    expect(ui.api.setEcoCodes).toHaveBeenCalledWith(
      "db-1",
      true,
      expect.any(Function),
    );
    expect(
      await screen.findByText(
        "399 games classified: 12 codes written, 300 already right, 80 other codes kept.",
      ),
    ).toBeVisible();
    expect(
      screen.getByText(
        "Of the 380 games that had a code, 300 already had the table's.",
      ),
    ).toBeVisible();
  });
});
