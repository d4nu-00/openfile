import { describe, it, expect } from "vitest";
import { act } from "@testing-library/react";
import { renderWorkbench, database } from "./harness";

describe("working copies", () => {
  it("keeps an edited game when replacement is cancelled", async () => {
    const ui = renderWorkbench({ dirty: true });
    await ui.user.click(ui.getByRole("button", { name: "Open Database" }));
    await ui.user.click(ui.getByRole("button", { name: "Cancel" }));
    expect(ui.api.openDatabase).not.toHaveBeenCalled();
    expect(ui.getByText("Unsaved changes")).toBeVisible();
    expect(ui.getByRole("button", { name: "e2 White pawn" })).toBeVisible();
  });
  it("keeps dirty state when save is cancelled or fails", async () => {
    const ui = renderWorkbench({ dirty: true });
    ui.api.chooseSave.mockResolvedValue(null);
    await ui.user.click(ui.getByRole("button", { name: "Save PGN" }));
    await ui.user.click(
      ui.getByRole("button", { name: "Export decoded content" }),
    );
    expect(ui.getByText("Unsaved changes")).toBeVisible();
    expect(ui.api.saveDocument).not.toHaveBeenCalled();
    ui.api.chooseSave.mockResolvedValue("/synthetic/save.pgn");
    ui.api.saveDocument.mockRejectedValue({ message: "Disk full" });
    await ui.user.click(ui.getByRole("button", { name: "Save PGN" }));
    await ui.user.click(
      ui.getByRole("button", { name: "Export decoded content" }),
    );
    expect(await ui.findByText("Disk full")).toBeVisible();
    expect(ui.getByText("Unsaved changes")).toBeVisible();
  });
  it("appends a legal branch and edits comments and NAGs at exact revisions", async () => {
    const ui = renderWorkbench();
    await ui.user.click(ui.getByRole("button", { name: "d2 White pawn" }));
    await ui.user.click(ui.getByRole("button", { name: "d4 empty" }));
    expect(ui.api.editDocument).toHaveBeenCalledWith(
      "doc-1",
      { kind: "appendMove", parent: 0, uci: "d2d4" },
      "0",
    );
    expect(ui.getByText("Unsaved changes")).toBeVisible();
    await ui.user.type(
      ui.getByRole("textbox", { name: "Comments" }),
      "A new branch",
    );
    await ui.user.click(ui.getByRole("button", { name: "Apply comments" }));
    expect(ui.api.editDocument).toHaveBeenLastCalledWith(
      "doc-1",
      { kind: "setComments", node: 4, comments: ["A new branch"] },
      "1",
    );
    await ui.user.type(ui.getByRole("textbox", { name: "NAGs" }), "1 14");
    await ui.user.click(ui.getByRole("button", { name: "Apply NAGs" }));
    expect(ui.api.editDocument).toHaveBeenLastCalledWith(
      "doc-1",
      { kind: "setNags", node: 4, nags: [1, 14] },
      "2",
    );
  });
  it("opens a PGN collection and selects its second game", async () => {
    const ui = renderWorkbench();
    await ui.user.click(ui.getByRole("button", { name: "Open PGN" }));
    await ui.user.selectOptions(
      ui.getByRole("combobox", { name: "PGN game" }),
      "1",
    );
    expect(
      await ui.findByRole("heading", { name: "Second — Beta" }),
    ).toBeVisible();
    expect(ui.api.loadPgnGame).toHaveBeenLastCalledWith(
      "pgn-1",
      1,
      expect.any(String),
    );
  });
  it("preserves the current game after a PGN parse failure", async () => {
    const ui = renderWorkbench();
    ui.api.openPgn.mockRejectedValue({ message: "Invalid PGN" });
    await ui.user.click(ui.getByRole("button", { name: "Open PGN" }));
    expect(await ui.findByText("Invalid PGN")).toBeVisible();
    expect(ui.getByRole("heading", { name: "Alpha — Beta" })).toBeVisible();
  });
});

it("requires an explicit promotion and sends the chosen legal move", async () => {
  const ui = renderWorkbench({
    position: {
      requestId: "initial",
      documentId: "doc-1",
      nodeId: 0,
      fen: "7k/P7/8/8/8/8/8/7K w - - 0 1",
      turn: "white",
      legalUci: ["a7a8q", "a7a8r", "a7a8b", "a7a8n"],
    },
  });
  await ui.user.click(ui.getByRole("button", { name: "a7 White pawn" }));
  await ui.user.click(ui.getByRole("button", { name: "a8 empty" }));
  expect(ui.api.editDocument).not.toHaveBeenCalled();
  await ui.user.click(ui.getByRole("button", { name: "Knight" }));
  expect(ui.api.editDocument).toHaveBeenCalledWith(
    "doc-1",
    { kind: "appendMove", parent: 0, uci: "a7a8n" },
    "0",
  );
});
it("cancels native window closing and permits it after discarding", async () => {
  const ui = renderWorkbench({ dirty: true });
  void ui.api.watchClose.mock.calls[0][0]();
  await ui.user.click(await ui.findByRole("button", { name: "Cancel" }));
  expect(ui.api.finishClose).not.toHaveBeenCalled();
  void ui.api.watchClose.mock.calls[0][0]();
  await ui.user.click(await ui.findByRole("button", { name: "Discard" }));
  expect(ui.api.finishClose).toHaveBeenCalled();
});
it("rejects a stale edit without changing the working game", async () => {
  const ui = renderWorkbench();
  ui.api.editDocument.mockRejectedValue({ message: "Stale revision" });
  await ui.user.click(ui.getByRole("button", { name: "d2 White pawn" }));
  await ui.user.click(ui.getByRole("button", { name: "d4 empty" }));
  expect(await ui.findByText("Stale revision")).toBeVisible();
  expect(ui.queryByText("Unsaved changes")).toBeNull();
  expect(ui.getByRole("button", { name: "d2 White pawn" })).toBeVisible();
});
it("discards the working copy only after a replacement database opens", async () => {
  const ui = renderWorkbench({ dirty: true });
  await ui.user.click(ui.getByRole("button", { name: "Open Database" }));
  await ui.user.click(ui.getByRole("button", { name: "Discard" }));
  expect(ui.api.openDatabase).toHaveBeenCalled();
  expect(ui.queryByText("Unsaved changes")).toBeNull();
  expect(ui.api.closeDocument).toHaveBeenCalledWith("doc-1");
});
it("allows editing a new PGN while an older database open is still pending", async () => {
  const ui = renderWorkbench();
  let release: (value: any) => void = () => {};
  ui.api.openDatabase.mockImplementationOnce(
    (_path, requestId) =>
      new Promise((resolve) => {
        release = () =>
          resolve({ ...database, requestId, databaseId: "old-opening" });
      }),
  );
  await ui.user.click(ui.getByRole("button", { name: "Open Database" }));
  await ui.user.click(ui.getByRole("button", { name: "Open PGN" }));
  await ui.findByRole("heading", { name: "First — Beta" });
  await ui.user.click(ui.getByRole("button", { name: "d2 White pawn" }));
  await ui.user.click(ui.getByRole("button", { name: "d4 empty" }));
  expect(ui.api.editDocument).toHaveBeenCalled();
  release(undefined);
});
it("prevents overlapping save pickers and replacement while a save is pending", async () => {
  const ui = renderWorkbench({ dirty: true });
  let finish!: (result: { savedRevision: string; destination: string }) => void;
  ui.api.saveDocument.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  await ui.user.click(ui.getByRole("button", { name: "Save PGN" }));
  await ui.user.click(
    await ui.findByRole("button", { name: "Export decoded content" }),
  );
  expect(ui.getByRole("button", { name: "Save PGN" })).toBeDisabled();
  await ui.user.click(ui.getByRole("button", { name: "Save PGN" }));
  await ui.user.click(ui.getByRole("button", { name: "Open PGN" }));
  expect(ui.api.chooseSave).toHaveBeenCalledTimes(1);
  expect(ui.api.openPgn).not.toHaveBeenCalled();
  await act(async () =>
    finish({ savedRevision: "0", destination: "/synthetic/save.pgn" }),
  );
  expect(ui.getByRole("button", { name: "Save PGN" })).toBeEnabled();
});
