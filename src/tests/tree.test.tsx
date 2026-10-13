import { describe, it, expect } from "vitest";
import { waitFor, within } from "@testing-library/react";
import { renderWorkbench } from "./harness";

describe("opening book", () => {
  it("lists the moves of the position and plays them", async () => {
    const ui = renderWorkbench();
    await ui.user.click(ui.getByRole("tab", { name: "Book" }));
    const book = await ui.findByLabelText("Opening book");
    expect(
      await within(book).findByText("10 games · 1 end here · Opening tree"),
    ).toBeVisible();
    expect(ui.api.openingTree).toHaveBeenCalledWith(
      "db-1",
      {
        startFen: expect.stringContaining("w KQkq"),
        moves: [],
      },
      expect.any(String),
    );
    const e4 = within(book)
      .getByRole("button", { name: "Play e4" })
      .closest("tr")!;
    expect(within(e4).getByText("60.0%")).toBeVisible();
    expect(within(e4).getByText("2535")).toBeVisible();
    expect(within(e4).getByText("2024")).toBeVisible();

    // An existing continuation is selected, not added again.
    await ui.user.click(within(book).getByRole("button", { name: "Play e4" }));
    expect(ui.api.positionAt).toHaveBeenLastCalledWith(
      "doc-1",
      1,
      expect.any(String),
    );
    expect(ui.api.editDocument).not.toHaveBeenCalled();
    // The book follows the board.
    await waitFor(() =>
      expect(ui.api.openingTree).toHaveBeenLastCalledWith(
        "db-1",
        expect.objectContaining({ moves: ["e2e4"] }),
        expect.any(String),
      ),
    );
  });

  it("adds a book move that is not in the game", async () => {
    const ui = renderWorkbench();
    await ui.user.keyboard("{Control>}{F12}{/Control}");
    const book = await ui.findByLabelText("Opening book");
    await ui.user.click(
      await within(book).findByRole("button", { name: "Play d4" }),
    );
    expect(ui.api.editDocument).toHaveBeenCalledWith(
      "doc-1",
      { kind: "appendMove", parent: 0, uci: "d2d4" },
      "0",
    );
  });

  it("offers to build a missing tree, and builds it from Maintenance", async () => {
    const ui = renderWorkbench();
    ui.api.openingTree.mockImplementation(
      async (_db: string, _line: unknown, requestId: string) => ({
        requestId,
        status: "noTree",
        games: 0,
        ended: 0,
        moves: [],
      }),
    );
    await ui.user.click(ui.getByRole("tab", { name: "Book" }));
    expect(
      await ui.findByText("This database has no opening tree yet."),
    ).toBeVisible();
    await ui.user.click(
      ui.getByRole("button", { name: "Create Opening Tree" }),
    );
    expect(ui.api.createOpeningTree).toHaveBeenCalledWith(
      "db-1",
      expect.any(Function),
    );
    expect(await ui.findByText("Opening tree is complete")).toBeVisible();

    await ui.user.click(ui.getByRole("tab", { name: /^Database/ }));
    await ui.user.click(ui.getByRole("tab", { name: "Maintenance" }));
    await ui.user.click(
      ui.getByRole("button", { name: "Delete Opening Tree" }),
    );
    expect(ui.api.deleteOpeningTree).toHaveBeenCalledWith("db-1");
  });
});

describe("reference search", () => {
  it("lists games reaching the board and opens them there", async () => {
    const ui = renderWorkbench();
    await ui.user.click(ui.getByRole("tab", { name: "Reference" }));
    const pane = await ui.findByLabelText("Reference search");
    expect(
      await within(pane).findByText("2 games from the opening tree"),
    ).toBeVisible();
    expect(ui.api.readReferencePage).toHaveBeenCalledWith(
      "db-1",
      "1",
      200,
      expect.any(String),
    );
    const row = within(pane)
      .getByRole("button", { name: "Load entry 12" })
      .closest("tr")!;
    await ui.user.dblClick(row);
    expect(ui.api.loadEntry).toHaveBeenLastCalledWith(
      "db-1",
      "12",
      expect.any(String),
    );
    await ui.findByText(/Entry 12 ·/);
    expect(ui.api.positionAt).toHaveBeenLastCalledWith(
      expect.any(String),
      3,
      expect.any(String),
    );
  });

  it("shows as an extra pane and stops its search when hidden", async () => {
    const ui = renderWorkbench();
    await ui.user.keyboard("{Control>}{Shift>}{F7}{/Shift}{/Control}");
    expect(await ui.findByLabelText("Reference pane")).toBeVisible();
    await waitFor(() => expect(ui.api.referenceSearch).toHaveBeenCalled());
    await ui.user.keyboard("{Control>}{Shift>}{F7}{/Shift}{/Control}");
    expect(ui.queryByLabelText("Reference pane")).toBeNull();
    expect(ui.api.cancelReference).toHaveBeenCalledWith("db-1");
  });
});
