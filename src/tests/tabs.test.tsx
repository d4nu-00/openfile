import { describe, it, expect } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { renderWorkbench, selected } from "./harness";
import {
  addTab,
  promotePreview,
  refocus,
  removeTab,
  tabGames,
  type WorkbenchState,
} from "../state/workbench";
import type { Edit, GameResponse } from "../api/types";

/** Each entry is its own document, "W<n> — B<n>", edited on its own. */
function distinctGames(ui: ReturnType<typeof renderWorkbench>) {
  const revisions = new Map<string, number>();
  ui.api.loadEntry.mockImplementation(
    async (_db: string, ordinal: string, requestId: string) =>
      ({
        ...selected,
        documentId: `doc-e${ordinal}`,
        requestId,
        summary: { ...selected.summary, ordinal },
        game: {
          ...selected.game!,
          tags: { White: `W${ordinal}`, Black: `B${ordinal}`, Result: "*" },
        },
      }) as GameResponse,
  );
  ui.api.editDocument.mockImplementation(
    async (documentId: string, edit: Edit) => {
      const revision = (revisions.get(documentId) ?? 0) + 1;
      revisions.set(documentId, revision);
      const game = structuredClone(selected.game!);
      game.tags = { White: documentId, Black: "?", Result: "*" };
      if (edit.kind === "setComments")
        game.nodes[edit.node].comments = edit.comments;
      return { documentId, game, revision: String(revision), dirty: true };
    },
  );
}

async function openEntry(ui: ReturnType<typeof renderWorkbench>, n: number) {
  await ui.user.click(screen.getByRole("tab", { name: /^Database/ }));
  await ui.user.dblClick(
    await screen.findByRole("button", { name: `Load entry ${n}` }),
  );
  await screen.findByRole("tab", { name: new RegExp(`W${n} — B${n}`) });
}
const gameTabs = () =>
  screen
    .getAllByRole("tab")
    .filter((t) => /—/.test(t.textContent ?? ""))
    .map((t) => t.textContent?.replace("●", "").trim());

describe("game tabs", () => {
  it("opens each game in its own tab and keeps each tab's game", async () => {
    const ui = renderWorkbench({ clean: true });
    distinctGames(ui);
    await ui.user.click(screen.getByRole("button", { name: "Open Database" }));
    await openEntry(ui, 3);
    await openEntry(ui, 5);
    expect(gameTabs()).toEqual(["W3 — B3", "W5 — B5"]);
    // Tab 3 gets a comment and unsaved changes; tab 5 stays clean.
    await ui.user.click(screen.getByRole("tab", { name: /W3 — B3/ }));
    expect(screen.getByRole("heading", { name: "W3 — B3" })).toBeVisible();
    await ui.user.type(
      screen.getByRole("textbox", { name: "Comments" }),
      "Mine",
    );
    await ui.user.click(screen.getByRole("button", { name: "Apply comments" }));
    await screen.findAllByTitle("Unsaved changes");
    await ui.user.click(screen.getByRole("tab", { name: /W5 — B5/ }));
    expect(screen.queryByText("Unsaved changes")).toBeNull();
    expect(ui.api.closeDocument).not.toHaveBeenCalled();

    // Closing the edited tab asks first; Cancel keeps it.
    await ui.user.click(
      screen.getByRole("button", { name: /Close game tab doc-e3/ }),
    );
    await ui.user.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(gameTabs()).toHaveLength(2);
    await ui.user.click(
      screen.getByRole("button", { name: /Close game tab doc-e3/ }),
    );
    await ui.user.click(await screen.findByRole("button", { name: "Discard" }));
    expect(ui.api.closeDocument).toHaveBeenCalledWith("doc-e3");
    expect(gameTabs()).toEqual(["W5 — B5"]);
    // The game shown before closing is shown again.
    expect(screen.getByRole("heading", { name: "W5 — B5" })).toBeVisible();
  });

  it("reuses the least recently used tab at the most tabs", async () => {
    const ui = renderWorkbench({ clean: true });
    distinctGames(ui);
    await ui.user.keyboard("{Control>}{Alt>}o{/Alt}{/Control}");
    const options = await screen.findByRole("dialog", { name: "Options" });
    const field = within(options).getByRole("spinbutton", {
      name: "Maximum game tabs",
    });
    await ui.user.clear(field);
    await ui.user.type(field, "2");
    await ui.user.click(within(options).getByRole("button", { name: "OK" }));
    expect(ui.api.setSettings).toHaveBeenCalledWith(
      expect.objectContaining({ maxGameTabs: 2 }),
    );
    await ui.user.click(screen.getByRole("button", { name: "Open Database" }));
    await openEntry(ui, 3);
    await openEntry(ui, 5);
    await openEntry(ui, 7);
    // Tab 3 was used least recently and had no unsaved changes.
    expect(gameTabs()).toEqual(["W5 — B5", "W7 — B7"]);
    expect(ui.api.closeDocument).toHaveBeenCalledWith("doc-e3");
    // Opening a game that has a tab shows that tab.
    await openEntry(ui, 5);
    expect(gameTabs()).toEqual(["W5 — B5", "W7 — B7"]);
    expect(
      screen.getByRole("tab", { name: /W5 — B5/, selected: true }),
    ).toBeVisible();
  });

  it("asks about every tab with unsaved changes before quitting", async () => {
    const ui = renderWorkbench({ clean: true });
    distinctGames(ui);
    await ui.user.click(screen.getByRole("button", { name: "Open Database" }));
    for (const n of [3, 5]) {
      await openEntry(ui, n);
      await ui.user.type(
        screen.getByRole("textbox", { name: "Comments" }),
        "x",
      );
      await ui.user.click(
        screen.getByRole("button", { name: "Apply comments" }),
      );
    }
    await waitFor(() =>
      expect(screen.getAllByTitle("Unsaved changes")).toHaveLength(2),
    );
    const quit = ui.api.watchClose.mock.calls[0][0];
    // Discard the first, cancel the second: the app stays open.
    void quit();
    await ui.user.click(await screen.findByRole("button", { name: "Discard" }));
    await ui.user.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(ui.api.finishClose).not.toHaveBeenCalled();
    // Discarding both quits.
    void quit();
    await ui.user.click(await screen.findByRole("button", { name: "Discard" }));
    await ui.user.click(await screen.findByRole("button", { name: "Discard" }));
    await waitFor(() => expect(ui.api.finishClose).toHaveBeenCalled());
  });

  it("opens new games in new tabs", async () => {
    const ui = renderWorkbench();
    expect(gameTabs()).toEqual(["Alpha — Beta"]);
    await ui.user.click(screen.getByRole("button", { name: "New Game" }));
    await waitFor(() => expect(gameTabs()).toHaveLength(2));
    expect(ui.api.closeDocument).not.toHaveBeenCalled();
  });
});

describe("tab state", () => {
  const game = (id: string) =>
    ({ ...selected, documentId: id }) as GameResponse;
  it("moves games between the shown fields and their tabs", () => {
    let s: WorkbenchState = { document: game("preview") };
    s = promotePreview(s);
    expect(s.focus).toBeDefined();
    expect(s.document?.documentId).toBe("preview");
    expect(s.preview?.document).toBeUndefined();
    const first = s.focus!;
    s = addTab(s, { document: game("second") });
    const second = s.focus!;
    expect(tabGames(s).map((t) => t.document?.documentId)).toEqual([
      "preview",
      "second",
    ]);
    s = refocus(s, first);
    expect(s.document?.documentId).toBe("preview");
    s = refocus(s, undefined);
    expect(s.document).toBeUndefined();
    s = refocus(s, second);
    s = removeTab(s, second);
    // The last used remaining tab is shown.
    expect(s.focus).toBe(first);
    s = removeTab(s, first);
    expect(s.focus).toBeUndefined();
    expect(tabGames(s)).toEqual([]);
  });
});

describe("tablebases", () => {
  it("shows the tablebase result and plays a move from it", async () => {
    const ui = renderWorkbench();
    ui.api.probeTablebase.mockImplementation(
      async (_fen: string, requestId: string) => ({
        requestId,
        status: "found",
        pieces: 3,
        maxPieces: 5,
        outcome: "win",
        dtz: 19,
        rounded: false,
        moves: [
          {
            uci: "e2e4",
            san: "e4",
            outcome: "win",
            dtz: -18,
            rounded: false,
            zeroing: true,
          },
          {
            uci: "d2d4",
            san: "d4",
            outcome: "draw",
            rounded: false,
            zeroing: true,
          },
        ],
      }),
    );
    await ui.user.click(screen.getByRole("tab", { name: "Tablebase" }));
    const pane = await screen.findByLabelText("Tablebase");
    expect(await within(pane).findByText("White wins · DTZ 19")).toBeVisible();
    expect(within(pane).getByText("Draw")).toBeVisible();
    await ui.user.click(within(pane).getByRole("button", { name: "Play e4" }));
    // e4 is already in the game: it is selected, not added again.
    expect(ui.api.editDocument).not.toHaveBeenCalled();
  });

  it("keeps the tablebase folder chosen in Options", async () => {
    const ui = renderWorkbench();
    await ui.user.keyboard("{Control>}{Alt>}o{/Alt}{/Control}");
    const options = await screen.findByRole("dialog", { name: "Options" });
    await ui.user.click(
      within(options).getByRole("button", { name: "Choose folder…" }),
    );
    expect(await within(options).findByText("/synthetic/syzygy")).toBeVisible();
    await ui.user.click(within(options).getByRole("button", { name: "OK" }));
    expect(ui.api.setSettings).toHaveBeenCalledWith(
      expect.objectContaining({ tablebaseDir: "/synthetic/syzygy" }),
    );
    await ui.user.click(screen.getByRole("tab", { name: "Tablebase" }));
    expect(await screen.findByText(/No tablebases/)).toBeVisible();
  });
});
