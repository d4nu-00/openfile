import { describe, it, expect } from "vitest";
import { renderWorkbench } from "./harness";

describe("database writing", () => {
  it("creates a database and saves the game into it", async () => {
    const ui = renderWorkbench({ dirty: true });
    // Without an editable database, Save exports PGN.
    ui.api.chooseSave.mockResolvedValue(null);
    await ui.user.keyboard("{Control>}s{/Control}");
    await ui.user.click(
      ui.getByRole("button", { name: "Export decoded content" }),
    );
    expect(ui.api.saveGame).not.toHaveBeenCalled();
    expect(ui.api.chooseSave).toHaveBeenCalled();

    await ui.user.click(ui.getByRole("tab", { name: /Database/ }));
    await ui.user.click(ui.getByRole("button", { name: "New" }));
    // The unsaved game is guarded first.
    await ui.user.click(ui.getByRole("button", { name: "Discard" }));
    expect(ui.api.createDatabase).toHaveBeenCalledWith(
      "/synthetic/new.2cbh",
      expect.any(String),
    );
    expect(await ui.findByText(/0 entries · editable/)).toBeVisible();
  });

  it("saves, replaces and deletes in a database opened for editing", async () => {
    const ui = renderWorkbench();
    ui.api.chooseDatabase.mockResolvedValue("/synthetic/edit.2cbh");
    await ui.user.click(ui.getByRole("tab", { name: /Database/ }));
    await ui.user.click(ui.getByRole("button", { name: "Open for Editing" }));
    expect(ui.api.confirmEditing).toHaveBeenCalled();
    expect(ui.api.openDatabase).toHaveBeenCalledWith(
      "/synthetic/edit.2cbh",
      expect.any(String),
      true,
    );
    await ui.findByText(/401 entries · editable/);
    // Load entry 1, then save it as a new game and replace it.
    await ui.user.dblClick(
      await ui.findByRole("button", { name: "Load entry 1" }),
    );
    await ui.user.click(await ui.findByRole("button", { name: "Save" }));
    // Its original annotations are not decoded: saving asks first.
    await ui.user.click(ui.getByRole("button", { name: "Save without them" }));
    expect(ui.api.saveGame).toHaveBeenCalledWith("doc-1", "db-1", "0", true);
    expect(await ui.findByText("Saved as game 201")).toBeVisible();
    // Once saved, the omission is acknowledged and Replace does not ask.
    await ui.user.click(ui.getByRole("button", { name: "Replace" }));
    expect(ui.api.replaceGame).toHaveBeenCalledWith("doc-1", "0", false);

    await ui.user.click(ui.getByRole("tab", { name: /Database/ }));
    // Delete acts on the selected entry of the list.
    await ui.user.click(ui.getByRole("button", { name: "Load entry 1" }));
    await ui.findByText(/Entry 1 ·/);
    await ui.user.click(ui.getByRole("button", { name: "Delete" }));
    expect(ui.api.setDeleted).toHaveBeenCalledWith("db-1", "1", true);
    expect(await ui.findByText("Game 1 marked deleted")).toBeVisible();
  });

  it("keeps classic databases read-only", async () => {
    const ui = renderWorkbench();
    await ui.user.click(ui.getByRole("tab", { name: /Database/ }));
    await ui.user.click(ui.getByRole("button", { name: "Open for Editing" }));
    expect(ui.api.confirmEditing).not.toHaveBeenCalled();
    expect(ui.api.openDatabase).not.toHaveBeenCalled();
    expect(
      await ui.findByText(/classic .cbh databases open read-only/),
    ).toBeVisible();
  });
});
