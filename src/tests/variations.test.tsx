import { describe, expect, it } from "vitest";
import { screen, within } from "@testing-library/react";
import { mockRows, renderWorkbench, startFen } from "./harness";

const at = (nodeId: number) => ({
  requestId: "initial",
  documentId: "doc-1",
  nodeId,
  fen: startFen,
  turn: "white" as const,
  legalUci: [],
});

describe("variation tools", () => {
  it("exchanges, reorders and cleans up variations", async () => {
    // Node 3 is 1...c5, the first move of a variation.
    const ui = renderWorkbench({ position: at(3) });
    await ui.user.click(ui.getByRole("tab", { name: "Insert" }));
    await ui.user.click(ui.getByRole("button", { name: "Exchange Moves" }));
    expect(ui.api.editDocument).toHaveBeenLastCalledWith(
      "doc-1",
      { kind: "exchangeMoves", node: 3 },
      expect.any(String),
    );
    await ui.user.click(ui.getByRole("button", { name: "Reorder Variations" }));
    const dialog = await ui.findByRole("dialog", {
      name: "Reorder Variations",
    });
    expect(within(dialog).getByText("Including the mainline?")).toBeVisible();
    await ui.user.click(within(dialog).getByRole("button", { name: "No" }));
    expect(ui.api.editDocument).toHaveBeenLastCalledWith(
      "doc-1",
      { kind: "reorderVariations", node: expect.any(Number), mainLine: false },
      expect.any(String),
    );
    await ui.user.click(
      ui.getByRole("button", { name: "Clean up variations" }),
    );
    expect(ui.api.editDocument).toHaveBeenLastCalledWith(
      "doc-1",
      { kind: "cleanUpVariations" },
      expect.any(String),
    );
  });

  it("names a game's medals in the list", async () => {
    const ui = renderWorkbench({ clean: true });
    ui.api.readPage.mockImplementation(
      async (_db: string, start: string, limit: number, requestId: string) => ({
        requestId,
        entries: mockRows(Number(start), limit).map((e) =>
          e.ordinal === "1" ? { ...e, medals: ["Novelty", "Endgame"] } : e,
        ),
        next: undefined,
      }),
    );
    await ui.user.click(screen.getByRole("button", { name: "Open Database" }));
    expect(
      await screen.findByRole("img", { name: "Medals: Novelty, Endgame" }),
    ).toBeInTheDocument();
  });
});
