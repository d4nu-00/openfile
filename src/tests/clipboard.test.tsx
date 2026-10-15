import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWorkbench } from "./harness";
import {
  chooseRow,
  noSelection,
  rowRanges,
  selectAll,
  selectedCount,
} from "../state/listSelection";

describe("list selection", () => {
  it("selects single rows, toggles with Ctrl and extends with Shift", () => {
    let s = chooseRow(noSelection, 5);
    expect(s.ranges).toEqual([[5, 6]]);
    s = chooseRow(s, 9, { toggle: true, extend: false });
    expect(s.ranges).toEqual([
      [5, 6],
      [9, 10],
    ]);
    s = chooseRow(s, 6, { toggle: true, extend: false });
    expect(s.ranges).toEqual([
      [5, 7],
      [9, 10],
    ]);
    s = chooseRow(s, 5, { toggle: true, extend: false });
    expect(s.ranges).toEqual([
      [6, 7],
      [9, 10],
    ]);
    s = chooseRow(chooseRow(noSelection, 3), 1, {
      toggle: false,
      extend: true,
    });
    expect(s.ranges).toEqual([[1, 4]]);
    expect(rowRanges(s)).toEqual([{ start: "2", end: "5" }]);
    expect(selectedCount(selectAll(11_151_119))).toBe(11_151_119);
  });
});

describe("copy, paste and delete games", () => {
  it("copies a Shift-selected range and pastes it into an editable database", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(
      await screen.findByRole("button", { name: "Load entry 1" }),
    );
    await ui.user.keyboard("{Shift>}");
    await ui.user.click(screen.getByRole("button", { name: "Load entry 3" }));
    await ui.user.keyboard("{/Shift}");
    expect(await screen.findByText("3 selected")).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Load entry 2" }).closest("tr"),
    ).toHaveAttribute("aria-selected", "true");
    await ui.user.click(screen.getByRole("button", { name: "Copy" }));
    expect(ui.api.copyRows).toHaveBeenCalledWith("db-1", [
      { start: "1", end: "4" },
    ]);
    expect(await screen.findByText("3 games copied")).toBeVisible();
    // Paste needs a database opened for editing; the copy stays.
    expect(screen.getByRole("button", { name: "Paste" })).toBeDisabled();
    ui.api.chooseDatabase.mockResolvedValue("/synthetic/target.2cbh");
    await ui.user.click(
      screen.getByRole("button", { name: "Open for Editing" }),
    );
    await screen.findByRole("tab", { name: "Database target" });
    await ui.user.click(screen.getByRole("button", { name: "Paste" }));
    expect(ui.api.confirmPaste).toHaveBeenCalledWith("3");
    expect(ui.api.pasteGames).toHaveBeenCalledWith(
      "db-2",
      expect.any(Function),
    );
    expect(
      await screen.findByText("3 games pasted (1 without some annotations)"),
    ).toBeVisible();
    expect(
      screen.getByRole("tab", { name: "Database target" }),
    ).toHaveTextContent("404");
  });

  it("selects all and deletes the selection in an editable database", async () => {
    const ui = renderWorkbench({ clean: true });
    ui.api.chooseDatabase.mockResolvedValue("/synthetic/edit.2cbh");
    await ui.user.click(
      screen.getByRole("button", { name: "Open for Editing" }),
    );
    await screen.findByRole("tab", { name: "Database edit" });
    await screen.findByRole("button", { name: "Load entry 1" });
    await ui.user.click(screen.getByRole("button", { name: "Select All" }));
    expect(await screen.findByText("401 selected")).toBeVisible();
    await ui.user.click(screen.getByRole("button", { name: "Delete" }));
    expect(ui.api.deleteRows).toHaveBeenCalledWith("db-1", [
      { start: "1", end: "402" },
    ]);
    expect(await screen.findByText("401 games changed")).toBeVisible();
    // The list is read again after the change.
    await waitFor(() => expect(screen.queryByText("401 selected")).toBeNull());
  });
});

describe("convert to 2CBH", () => {
  it("estimates, converts the selection and opens the new database for editing", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(
      await screen.findByRole("button", { name: "Load entry 1" }),
    );
    await ui.user.keyboard("{Shift>}");
    await ui.user.click(screen.getByRole("button", { name: "Load entry 3" }));
    await ui.user.keyboard("{/Shift}");
    await ui.user.click(screen.getByRole("tab", { name: "Maintenance" }));
    await ui.user.click(
      screen.getByRole("button", { name: "Convert to 2CBH" }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Convert to 2CBH",
    });
    // The selection is offered first; the estimate is checked against it.
    expect(ui.api.convertEstimate).toHaveBeenCalledWith(
      "db-1",
      [{ start: "1", end: "4" }],
      "/dbs/Converted.2cbh",
    );
    expect(
      await within(dialog).findByText(
        "About 1 MB needed; 5.00 GB free on that disk.",
      ),
    ).toBeVisible();
    await ui.user.click(
      within(dialog).getByRole("radio", { name: /Whole database/ }),
    );
    await waitFor(() =>
      expect(ui.api.convertEstimate).toHaveBeenLastCalledWith(
        "db-1",
        null,
        "/dbs/Converted.2cbh",
      ),
    );
    await ui.user.click(
      await within(dialog).findByRole("button", { name: "Convert" }),
    );
    expect(ui.api.convertDatabase).toHaveBeenCalledWith(
      "db-1",
      null,
      "/dbs/Converted.2cbh",
      expect.any(Function),
    );
    expect(
      await screen.findByText(
        "400 games converted (2 marked deleted, 3 without some annotations, 4 text entries left out)",
      ),
    ).toBeVisible();
    expect(ui.api.openDatabase).toHaveBeenLastCalledWith(
      "/dbs/Converted.2cbh",
      expect.any(String),
      true,
    );
    expect(
      screen.queryByRole("dialog", { name: "Convert to 2CBH" }),
    ).toBeNull();
  });

  it("refuses to start when the estimate does not fit the disk", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    ui.api.convertEstimate.mockResolvedValue({
      games: "404",
      bytes: "9000000000",
      free: "4000000000",
      fits: false,
    });
    await screen.findByRole("button", { name: "Load entry 1" });
    await ui.user.click(screen.getByRole("tab", { name: "Maintenance" }));
    await ui.user.click(
      screen.getByRole("button", { name: "Convert to 2CBH" }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Convert to 2CBH",
    });
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "Not enough disk space",
    );
    expect(
      within(dialog).getByRole("button", { name: "Convert" }),
    ).toBeDisabled();
    expect(ui.api.convertDatabase).not.toHaveBeenCalled();
  });
});
