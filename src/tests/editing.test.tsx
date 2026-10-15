import { describe, it, expect } from "vitest";
import { waitFor, within } from "@testing-library/react";
import { renderWorkbench, startFen } from "./harness";

const at = (nodeId: number) => ({
  requestId: "initial",
  documentId: "doc-1",
  nodeId,
  fen: startFen,
  turn: "white" as const,
  legalUci: [],
});

describe("variation commands", () => {
  it("delete and promote the variation of the current move", async () => {
    // Node 3 (1...c5) is a variation of 1...e5.
    const ui = renderWorkbench({ position: at(3) });
    await ui.user.keyboard("{Alt>}{ArrowUp}{/Alt}");
    expect(ui.api.editDocument).toHaveBeenCalledWith(
      "doc-1",
      { kind: "promoteVariation", node: 3 },
      "0",
    );
    // The backend's choice of node is selected after a structural edit.
    await waitFor(() =>
      expect(ui.api.positionAt).toHaveBeenLastCalledWith(
        "doc-1",
        0,
        expect.any(String),
      ),
    );
  });

  it("are unavailable on the main line", async () => {
    const ui = renderWorkbench({ position: at(2) });
    await ui.user.click(ui.getByRole("tab", { name: "Insert" }));
    expect(ui.getByRole("button", { name: "Delete Variation" })).toBeDisabled();
    await ui.user.click(
      ui.getByRole("button", { name: "Delete Previous Moves" }),
    );
    expect(ui.api.editDocument).toHaveBeenCalledWith(
      "doc-1",
      { kind: "deletePrevious", node: 2 },
      "0",
    );
  });

  it("undoes and redoes", async () => {
    const ui = renderWorkbench({ position: at(1) });
    await ui.user.keyboard("{Control>}z{/Control}");
    expect(ui.api.editDocument).toHaveBeenLastCalledWith(
      "doc-1",
      { kind: "undo" },
      "0",
    );
    await ui.user.keyboard("{Control>}{Alt>}{Shift>}y{/Shift}{/Alt}{/Control}");
    await waitFor(() =>
      expect(ui.api.editDocument).toHaveBeenLastCalledWith(
        "doc-1",
        { kind: "redo" },
        "1",
      ),
    );
  });
});

describe("game data and text", () => {
  it("edits the game data with F2", async () => {
    const ui = renderWorkbench();
    await ui.user.keyboard("{F2}");
    const dialog = ui.getByRole("dialog", { name: "Game data" });
    const white = within(dialog).getByLabelText("White");
    expect(white).toHaveValue("Alpha");
    await ui.user.clear(white);
    await ui.user.type(white, "Kasparov, Garry");
    await ui.user.type(within(dialog).getByLabelText("Elo White"), "2851");
    await ui.user.click(within(dialog).getByLabelText("1-0"));
    await ui.user.click(within(dialog).getByRole("button", { name: "OK" }));
    expect(ui.api.editDocument).toHaveBeenCalledWith(
      "doc-1",
      {
        kind: "setTags",
        tags: expect.objectContaining({
          White: "Kasparov, Garry",
          WhiteElo: "2851",
          Result: "1-0",
        }),
      },
      "0",
    );
    await waitFor(() =>
      expect(ui.queryByRole("dialog", { name: "Game data" })).toBeNull(),
    );
  });

  it("writes text before a move", async () => {
    const ui = renderWorkbench({ position: at(1) });
    await ui.user.keyboard("{Control>}{Shift>}a{/Shift}{/Control}");
    const before = ui.getByLabelText("Text before move");
    expect(before).toHaveFocus();
    await ui.user.type(before, "The usual move");
    await ui.user.click(ui.getByRole("button", { name: "Apply text before" }));
    expect(ui.api.editDocument).toHaveBeenCalledWith(
      "doc-1",
      { kind: "setPreComments", node: 1, comments: ["The usual move"] },
      "0",
    );
  });

  it("sets symbols from the annotation palette", async () => {
    // 1.e4 carries "!" (NAG 1); "?" replaces it, as both are move marks.
    const ui = renderWorkbench({ position: at(1) });
    await ui.user.keyboard("{Control>}{Alt>}s{/Alt}{/Control}");
    const palette = ui.getByLabelText("Annotation palette");
    expect(
      within(palette).getByRole("button", { name: "Good move" }),
    ).toHaveAttribute("aria-pressed", "true");
    await ui.user.click(
      within(palette).getByRole("button", { name: "Mistake" }),
    );
    expect(ui.api.editDocument).toHaveBeenCalledWith(
      "doc-1",
      { kind: "setNags", node: 1, nags: [2] },
      "0",
    );
  });

  it("opens ChessBase's symbol popups for typed !, ? and =", async () => {
    // 1.e4 carries "!" (NAG 1), so the move popup starts on it.
    const ui = renderWorkbench({ position: at(1) });
    await ui.user.keyboard("?");
    const menu = ui.getByRole("listbox", { name: "Move symbol" });
    expect(
      within(menu).getByRole("option", { name: "Good move" }),
    ).toHaveAttribute("aria-selected", "true");
    // Down to "?" and choose it: it replaces "!".
    await ui.user.keyboard("{ArrowDown}{Enter}");
    expect(ui.api.editDocument).toHaveBeenCalledWith(
      "doc-1",
      { kind: "setNags", node: 1, nags: [2] },
      "0",
    );
    await waitFor(() =>
      expect(ui.queryByRole("listbox", { name: "Move symbol" })).toBeNull(),
    );
    // "=" offers evaluations; the move symbol is kept.
    await ui.user.keyboard("=");
    const evaluation = ui.getByRole("listbox", { name: "Evaluation" });
    await ui.user.click(
      within(evaluation).getByRole("option", { name: "Unclear" }),
    );
    expect(ui.api.editDocument).toHaveBeenLastCalledWith(
      "doc-1",
      { kind: "setNags", node: 1, nags: [2, 13] },
      "1",
    );
    // Escape closes without an edit; "(none)" clears the group.
    await ui.user.keyboard("$");
    await ui.user.keyboard("{Escape}");
    expect(ui.queryByRole("listbox", { name: "Prefix symbol" })).toBeNull();
    await ui.user.keyboard("!");
    await ui.user.keyboard("{Home}{Enter}");
    expect(ui.api.editDocument).toHaveBeenLastCalledWith(
      "doc-1",
      { kind: "setNags", node: 1, nags: [13] },
      "2",
    );
  });
});

describe("new games and the clipboard", () => {
  it("starts a new game", async () => {
    const ui = renderWorkbench();
    await ui.user.keyboard("{Control>}n{/Control}");
    expect(ui.api.newDocument).toHaveBeenCalledWith(null, expect.any(String));
    expect(await ui.findByText("New game")).toBeVisible();
  });

  it("sets up a position and reports an impossible one", async () => {
    const ui = renderWorkbench();
    await ui.user.keyboard("s");
    const dialog = ui.getByRole("dialog", { name: "Position setup" });
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Clear board" }),
    );
    expect(within(dialog).getByLabelText("FEN")).toHaveValue(
      "8/8/8/8/8/8/8/8 w KQkq - 0 1",
    );
    await ui.user.click(within(dialog).getByRole("button", { name: "OK" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "no kings",
    );
    // Place two kings: the palette tool, then the square.
    await ui.user.click(
      within(dialog).getByRole("radio", { name: "White king" }),
    );
    await ui.user.click(within(dialog).getByLabelText(/^e1 /));
    await ui.user.click(
      within(dialog).getByRole("radio", { name: "Black king" }),
    );
    await ui.user.click(within(dialog).getByLabelText(/^e8 /));
    await ui.user.click(
      within(dialog).getByRole("checkbox", { name: "White O-O" }),
    );
    await ui.user.click(within(dialog).getByRole("button", { name: "OK" }));
    expect(ui.api.newDocument).toHaveBeenLastCalledWith(
      "4k3/8/8/8/8/8/8/4K3 w Qkq - 0 1",
      expect.any(String),
    );
    await waitFor(() =>
      expect(ui.queryByRole("dialog", { name: "Position setup" })).toBeNull(),
    );
  });

  it("copies the game and pastes one", async () => {
    const ui = renderWorkbench();
    await ui.user.keyboard("{Control>}c{/Control}");
    expect(ui.api.documentPgn).toHaveBeenCalledWith("doc-1");
    await waitFor(() =>
      expect(ui.api.writeClipboard).toHaveBeenCalledWith(
        '[Result "*"]\n\n1. e4 *\n',
      ),
    );
    // An unreadable clipboard asks for the text.
    await ui.user.keyboard("{Control>}v{/Control}");
    const dialog = await ui.findByRole("dialog", { name: "Paste game" });
    await ui.user.type(within(dialog).getByLabelText("PGN"), "1. d4 *");
    await ui.user.click(within(dialog).getByRole("button", { name: "OK" }));
    await waitFor(() =>
      expect(ui.api.pasteGame).toHaveBeenCalledWith(
        "1. d4 *",
        expect.any(String),
      ),
    );
    // A readable one is pasted at once.
    ui.api.readClipboard.mockResolvedValueOnce("1. c4 *");
    await ui.user.keyboard("{Control>}v{/Control}");
    await waitFor(() =>
      expect(ui.api.pasteGame).toHaveBeenLastCalledWith(
        "1. c4 *",
        expect.any(String),
      ),
    );
  });
});
