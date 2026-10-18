import { within, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWorkbench } from "./harness";

describe("database start screen", () => {
  it("shows My Databases with no database open and opens a symbol", async () => {
    const ui = renderWorkbench({ clean: true, initialView: "database" });
    const screen = await ui.findByLabelText("Databases start screen");
    const symbols = await within(screen).findByRole("listbox", {
      name: "My Databases symbols",
    });
    const mega = within(symbols).getByRole("option", { name: /test database/ });
    expect(mega).toHaveTextContent("11,151,119 entries · CBH");
    // A database whose files are gone is shown dimmed and does not open.
    const gone = within(symbols).getByRole("option", { name: /Gone/ });
    expect(gone).toHaveClass("missing");
    await ui.user.dblClick(gone);
    expect(ui.api.openDatabase).not.toHaveBeenCalled();
    await ui.user.dblClick(mega);
    await waitFor(() =>
      expect(ui.api.openDatabase).toHaveBeenCalledWith(
        "/dbs/test database.cbh",
        expect.any(String),
      ),
    );
    // The opened database replaces the start screen; My Databases brings it back.
    await waitFor(() =>
      expect(ui.queryByLabelText("Databases start screen")).toBeNull(),
    );
    await ui.user.click(ui.getByRole("button", { name: "My Databases" }));
    expect(await ui.findByLabelText("Databases start screen")).toBeVisible();
  });

  it("removes symbols, sets types, lists folders and shows details", async () => {
    const ui = renderWorkbench({ clean: true, initialView: "database" });
    const screen = await ui.findByLabelText("Databases start screen");
    await ui.user.click(
      await within(screen).findByRole("option", { name: /test database/ }),
    );
    await ui.user.selectOptions(
      within(screen).getByLabelText("Database type"),
      "openings",
    );
    expect(ui.api.setDatabaseKind).toHaveBeenCalledWith(
      "/dbs/test database.cbh",
      "openings",
    );
    await ui.user.click(
      within(screen).getByRole("button", { name: "Remove Database Symbol" }),
    );
    expect(ui.api.unpinDatabase).toHaveBeenCalledWith("/dbs/test database.cbh");

    // Recent Databases: pin a database that is not in My Databases.
    await ui.user.click(
      within(screen).getByRole("button", { name: /Recent Databases/ }),
    );
    await ui.user.click(
      await within(screen).findByRole("option", { name: /Openings/ }),
    );
    await ui.user.click(
      within(screen).getByRole("button", { name: "Pin to My Databases" }),
    );
    expect(ui.api.pinDatabase).toHaveBeenCalledWith("/work/Openings.2cbh");

    // A folder shortcut lists the databases in its folder.
    await ui.user.click(within(screen).getByRole("button", { name: "work" }));
    expect(ui.api.folderDatabases).toHaveBeenCalledWith("/work");
    expect(
      await within(screen).findByRole("listbox", { name: "work symbols" }),
    ).toBeVisible();
    await ui.user.click(
      within(screen).getByRole("button", { name: "Add Folder Shortcut" }),
    );
    await waitFor(() =>
      expect(ui.api.addFolderShortcut).toHaveBeenCalledWith("/new/folder"),
    );
    await ui.user.click(
      within(screen).getByRole("button", {
        name: "Remove folder shortcut work",
      }),
    );
    expect(ui.api.removeFolderShortcut).toHaveBeenCalledWith("/work");

    // Details lists the symbols as a table; Folders hides the shortcuts.
    await ui.user.click(
      within(screen).getByRole("button", { name: /^My Databases/ }),
    );
    await ui.user.click(
      within(screen).getByRole("button", { name: "Details" }),
    );
    expect(
      await within(screen).findByRole("table", {
        name: "My Databases details",
      }),
    ).toBeVisible();
    await ui.user.click(ui.getByRole("button", { name: "Folders" }));
    expect(within(screen).queryByText("Add Folder Shortcut")).toBeNull();
  });
});
