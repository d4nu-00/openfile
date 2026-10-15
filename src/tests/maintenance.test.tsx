import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWorkbench } from "./harness";

async function editable() {
  const ui = renderWorkbench({ clean: true });
  ui.api.chooseDatabase.mockResolvedValue("/synthetic/edit.2cbh");
  await ui.user.click(screen.getByRole("button", { name: "Open for Editing" }));
  await screen.findByRole("tab", { name: "Database edit" });
  await ui.user.click(screen.getByRole("tab", { name: "Maintenance" }));
  return ui;
}

describe("database maintenance", () => {
  it("checks integrity and lists the problems", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await screen.findByRole("button", { name: "Load entry 1" });
    await ui.user.click(screen.getByRole("tab", { name: "Maintenance" }));
    await ui.user.click(
      screen.getByRole("button", { name: "Check Integrity" }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Check Integrity",
    });
    expect(
      await within(dialog).findByText(
        "Checked 404 entries: 400 games, 4 texts, 1 marked deleted.",
      ),
    ).toBeVisible();
    expect(
      within(dialog).getByText("1 problems: damagedMoves 1."),
    ).toBeVisible();
    expect(within(dialog).getByText("Entry 7: Block checksum")).toBeVisible();
    // Find Double Games and Remove Deleted Games need an editable database.
    expect(
      screen.getByRole("button", { name: "Find Double Games" }),
    ).toBeDisabled();
  });

  it("finds doubles with ChessBase's options and marks them", async () => {
    const ui = await editable();
    await ui.user.click(
      screen.getByRole("button", { name: "Find Double Games" }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Find Double Games",
    });
    expect(
      within(dialog).getByRole("radio", { name: "Keep better game" }),
    ).toBeChecked();
    await ui.user.click(
      within(within(dialog).getByRole("group", { name: "Names" })).getByRole(
        "radio",
        { name: "Exact" },
      ),
    );
    await ui.user.click(
      within(dialog).getByRole("checkbox", { name: "Ignore year" }),
    );
    await ui.user.click(within(dialog).getByRole("button", { name: "Find" }));
    expect(ui.api.findDoubleGames).toHaveBeenCalledWith(
      "db-1",
      expect.objectContaining({
        decision: "keepBetter",
        names: "exact",
        moves: "similar",
        ignoreResult: true,
        ignoreYear: true,
      }),
      expect.any(Function),
    );
    const result = await screen.findByRole("dialog", {
      name: "Find Double Games",
    });
    expect(
      await within(result).findByText(
        "Compared 404 entries: 2 doubles marked deleted.",
      ),
    ).toBeVisible();
    expect(
      within(result).getByText("Game 3 marked; game 2 kept."),
    ).toBeVisible();
  });

  it("removes deleted games after asking", async () => {
    const ui = await editable();
    await ui.user.click(
      screen.getByRole("button", { name: "Remove Deleted Games" }),
    );
    expect(ui.api.confirmRemoveDeleted).toHaveBeenCalled();
    const dialog = await screen.findByRole("dialog", {
      name: "Remove Deleted Games",
    });
    expect(
      await within(dialog).findByText(
        "4 deleted games removed; 400 entries kept.",
      ),
    ).toBeVisible();
    expect(
      screen.getByRole("tab", { name: "Database edit" }),
    ).toHaveTextContent("400");
  });
});
