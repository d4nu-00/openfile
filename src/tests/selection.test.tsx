import { it, expect } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { SelectionGate } from "../state/selection";
import { renderWorkbench, selected } from "./harness";
it("closes a stale database after its delayed page finishes", async () => {
  const ui = renderWorkbench({ clean: true });
  let resolve: (p: any) => void = () => {};
  let old = "";
  ui.api.openDatabase.mockImplementationOnce(async (_p, requestId) => ({
    requestId,
    databaseId: "db-old",
    entryCount: "1",
    companions: [],
    diagnostics: [],
  }));
  ui.api.openDatabase.mockImplementationOnce(async (_p, requestId) => ({
    requestId,
    databaseId: "db-current",
    entryCount: "2",
    companions: [],
    diagnostics: [],
  }));
  ui.api.readPage.mockImplementationOnce((_d, _s, _n, requestId) => {
    old = requestId;
    return new Promise((r) => {
      resolve = r;
    });
  });
  await ui.user.click(screen.getByRole("button", { name: "Open Database" }));
  await waitFor(() => expect(ui.api.readPage).toHaveBeenCalledTimes(1));
  await ui.user.click(screen.getByRole("button", { name: "Open Database" }));
  await waitFor(() => expect(screen.getAllByText("2 entries")).toHaveLength(2));
  resolve({ requestId: old, entries: [] });
  await waitFor(() =>
    expect(ui.api.closeDatabase).toHaveBeenCalledWith("db-old"),
  );
  expect(screen.getAllByText("2 entries")).toHaveLength(2);
});
it("rejects an older selection result", () => {
  const g = new SelectionGate();
  const old = g.begin();
  const current = g.begin();
  expect(g.accept(old)).toBe(false);
  expect(g.accept(current)).toBe(true);
});
it("closes stale successful game handles without replacing the current game", async () => {
  const ui = renderWorkbench({ initialView: "database" });
  let resolve: (value: typeof selected) => void = () => {};
  let oldId = "";
  ui.api.loadEntry.mockImplementationOnce((_db, _ordinal, requestId) => {
    oldId = requestId;
    return new Promise((r) => {
      resolve = r;
    });
  });
  ui.api.loadEntry.mockImplementationOnce(async (_db, _ordinal, requestId) => ({
    ...selected,
    requestId,
    documentId: "doc-new",
    game: { ...selected.game!, tags: { White: "New", Black: "Game" } },
  }));
  await ui.user.click(
    await screen.findByRole("button", { name: "Load entry 1" }),
  );
  await ui.user.click(screen.getByRole("button", { name: "Load entry 2" }));
  expect(
    await screen.findByRole("heading", { name: "New — Game" }),
  ).toBeVisible();
  resolve({ ...selected, requestId: oldId, documentId: "doc-stale" });
  await waitFor(() =>
    expect(ui.api.closeDocument).toHaveBeenCalledWith("doc-stale"),
  );
  expect(screen.getByRole("heading", { name: "New — Game" })).toBeVisible();
});
it("does not display a stale position response", async () => {
  const ui = renderWorkbench();
  let resolve: (
    v: Awaited<ReturnType<typeof ui.api.positionAt>>,
  ) => void = () => {};
  let id = "";
  ui.api.positionAt.mockImplementationOnce((_doc, _node, requestId) => {
    id = requestId;
    return new Promise((r) => {
      resolve = r;
    });
  });
  await ui.user.click(screen.getByRole("button", { name: "e4" }));
  await ui.user.click(screen.getByRole("button", { name: "e5" }));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "e5" })).toHaveAttribute(
      "aria-current",
      "step",
    ),
  );
  resolve({
    requestId: id,
    documentId: "doc-1",
    nodeId: 1,
    fen: "8/8/8/8/8/8/8/8 w - - 0 1",
    turn: "white",
    legalUci: [],
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "e5" })).toHaveAttribute(
      "aria-current",
      "step",
    ),
  );
});
