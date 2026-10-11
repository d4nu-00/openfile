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
  await ui.user.click(screen.getByRole("button", { name: "Open database" }));
  await ui.user.click(
    await screen.findByRole("button", { name: "Load entry 1" }),
  );
  expect(await screen.findByText("Alpha — Beta")).toBeVisible();
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
  await ui.user.click(screen.getByRole("button", { name: "Flip board" }));
  expect(screen.getByRole("grid", { name: "Chess board" })).toHaveAttribute(
    "data-flipped",
    "true",
  );
  await ui.user.click(screen.getByRole("button", { name: "Next page" }));
  expect(
    await screen.findByRole("button", { name: "Load entry 201" }),
  ).toBeVisible();
  await ui.user.click(screen.getByRole("button", { name: "Previous page" }));
  expect(
    await screen.findByRole("button", { name: "Load entry 1" }),
  ).toBeVisible();
});
it("labels text and deleted entries and preserves the game on a failed load", async () => {
  const ui = renderWorkbench();
  expect(screen.getByText("Deleted")).toBeVisible();
  expect(screen.getByText("Text entry")).toBeVisible();
  ui.api.loadEntry.mockRejectedValueOnce({ message: "Broken source" });
  await ui.user.click(screen.getByRole("button", { name: "Load entry 2" }));
  expect(await screen.findByText("Broken source")).toBeVisible();
  expect(screen.getByText("Alpha — Beta")).toBeVisible();
});
it("explains unavailable replay and keeps the prior game", async () => {
  const ui = renderWorkbench();
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
  await ui.user.click(screen.getByRole("button", { name: "Load entry 2" }));
  expect(await screen.findByText("CBG companion is unavailable")).toBeVisible();
  expect(screen.getByText("Alpha — Beta")).toBeVisible();
});
it("keyboard navigation ignores editable fields", async () => {
  const ui = renderWorkbench();
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
it("uses decimal arithmetic for ordinal jumps and exposes keyboard pane resizing", async () => {
  const ui = renderWorkbench();
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
  const splitter = screen.getByRole("separator", {
    name: "Resize analysis pane",
  });
  const before = splitter.getAttribute("aria-valuenow");
  splitter.focus();
  await ui.user.keyboard("{ArrowUp}");
  expect(splitter.getAttribute("aria-valuenow")).not.toBe(before);
});
