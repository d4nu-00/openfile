import { screen, waitFor } from "@testing-library/react";
import { it, expect } from "vitest";
import { renderWorkbench, selected, summary } from "./harness";
it("exposes all 64 squares as accessible buttons", () => {
  renderWorkbench();
  expect(screen.getByRole("button", { name: "e2 White pawn" })).toBeVisible();
  expect(screen.getByRole("button", { name: "a1 White rook" })).toHaveClass(
    "dark",
  );
});
it("opens, pages, loads, visits variations, steps back and flips", async () => {
  const ui = renderWorkbench({ clean: true });
  await ui.user.click(screen.getByRole("button", { name: "Open Database" }));
  await ui.user.click(
    await screen.findByRole("button", { name: "Load entry 1" }),
  );
  expect(
    await screen.findByRole("heading", { name: "Alpha — Beta" }),
  ).toBeVisible();
  await ui.user.click(screen.getByRole("button", { name: "e4" }));
  await waitFor(() =>
    expect(ui.api.positionAt).toHaveBeenLastCalledWith(
      "doc-1",
      1,
      expect.any(String),
    ),
  );
  await ui.user.click(screen.getByRole("button", { name: "c5" }));
  await ui.user.click(screen.getByRole("button", { name: "Previous move" }));
  await waitFor(() =>
    expect(ui.api.positionAt).toHaveBeenLastCalledWith(
      "doc-1",
      1,
      expect.any(String),
    ),
  );
  await ui.user.click(screen.getByRole("button", { name: /Open game/ }));
  await ui.user.click(screen.getByRole("tab", { name: "Board" }));
  await ui.user.click(screen.getByRole("button", { name: "Flip board" }));
  expect(screen.getByRole("grid", { name: "Chess board" })).toHaveAttribute(
    "data-flipped",
    "true",
  );
  await ui.user.click(screen.getByRole("tab", { name: /Database/ }));
  // Page Down scrolls the list by a screen (30 rows before measuring).
  await ui.user.keyboard("{PageDown}");
  expect(
    await screen.findByRole("button", { name: "Load entry 31" }),
  ).toBeVisible();
  expect(screen.queryByRole("button", { name: "Load entry 1" })).toBeNull();
  await ui.user.keyboard("{PageUp}");
  expect(
    await screen.findByRole("button", { name: "Load entry 1" }),
  ).toBeVisible();
});
it("labels text and deleted entries and preserves the game on a failed load", async () => {
  const ui = renderWorkbench({ initialView: "database" });
  expect(await screen.findByText("Deleted")).toBeVisible();
  expect(screen.getByText("Text entry")).toBeVisible();
  ui.api.loadEntry.mockRejectedValueOnce({ message: "Broken source" });
  await ui.user.click(screen.getByRole("button", { name: "Load entry 2" }));
  expect(await screen.findByText("Broken source")).toBeVisible();
  expect(screen.getByRole("heading", { name: "Alpha — Beta" })).toBeVisible();
});
it("explains unavailable replay and keeps the prior game", async () => {
  const ui = renderWorkbench({ initialView: "database" });
  ui.api.loadEntry.mockImplementationOnce(async (_db, _ordinal, requestId) => ({
    ...selected,
    documentId: undefined,
    game: undefined,
    movesStatus: "missing",
    diagnostics: [
      { code: "missingMoves", message: "CBG companion is unavailable" },
    ],
    summary: { ...summary, ordinal: "2" },
    requestId,
  }));
  await ui.user.click(
    await screen.findByRole("button", { name: "Load entry 2" }),
  );
  expect(await screen.findByText("CBG companion is unavailable")).toBeVisible();
  expect(screen.getByRole("heading", { name: "Alpha — Beta" })).toBeVisible();
});
it("keyboard navigation ignores editable fields", async () => {
  const ui = renderWorkbench({ initialView: "database" });
  await ui.user.keyboard("{ArrowRight}");
  await waitFor(() =>
    expect(ui.api.positionAt).toHaveBeenLastCalledWith(
      "doc-1",
      1,
      expect.any(String),
    ),
  );
  await ui.user.keyboard("{End}");
  await waitFor(() =>
    expect(ui.api.positionAt).toHaveBeenLastCalledWith(
      "doc-1",
      2,
      expect.any(String),
    ),
  );
  await ui.user.keyboard("{Home}");
  await waitFor(() =>
    expect(ui.api.positionAt).toHaveBeenLastCalledWith(
      "doc-1",
      0,
      expect.any(String),
    ),
  );
  const count = ui.api.positionAt.mock.calls.length;
  await ui.user.click(screen.getByLabelText("Entry number"));
  await ui.user.keyboard("{ArrowRight}{Home}");
  expect(ui.api.positionAt).toHaveBeenCalledTimes(count);
});
it("uses decimal arithmetic for ordinal jumps", async () => {
  const ui = renderWorkbench({ initialView: "database" });
  const input = screen.getByLabelText("Entry number");
  await ui.user.clear(input);
  await ui.user.type(input, "9007199254740993");
  await ui.user.click(screen.getByRole("button", { name: "Go to entry" }));
  await waitFor(() =>
    expect(ui.api.loadEntry).toHaveBeenCalledWith(
      "db-1",
      "9007199254740993",
      expect.any(String),
    ),
  );
});
it("exposes a focusable separator for resizing the analysis pane", () => {
  renderWorkbench();
  const splitter = screen.getByRole("separator", {
    name: "Resize analysis pane",
  });
  // Resizing itself needs real layout and is checked in the browser.
  expect(splitter).toHaveAttribute("tabindex", "0");
  expect(splitter).toHaveAttribute("aria-valuenow");
});
it("loads a double-clicked game once", async () => {
  const ui = renderWorkbench({ clean: true });
  await ui.user.click(screen.getByRole("button", { name: "Open Database" }));
  await ui.user.dblClick(
    await screen.findByRole("button", { name: "Load entry 3" }),
  );
  await screen.findByText(/Entry 3 ·/);
  // Both clicks and the double-click ask for the same game: one load, and
  // the game moves into a tab of its own.
  const loads = () =>
    ui.api.loadEntry.mock.calls.filter((c: unknown[]) => c[1] === "3");
  expect(loads()).toHaveLength(1);
  expect(
    screen.getByRole("tab", { name: /Alpha — Beta/, selected: true }),
  ).toBeVisible();
  // Back in the list, the preview is empty: a click loads the game there,
  // and a second click on it loads nothing.
  await ui.user.click(screen.getByRole("tab", { name: /^Database/ }));
  await ui.user.click(screen.getByRole("button", { name: "Load entry 3" }));
  await ui.user.click(screen.getByRole("button", { name: "Load entry 3" }));
  await waitFor(() => expect(loads()).toHaveLength(2));
});
