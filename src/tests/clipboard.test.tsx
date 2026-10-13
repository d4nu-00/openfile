import { screen, waitFor } from "@testing-library/react";
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
