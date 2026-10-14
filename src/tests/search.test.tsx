import { describe, it, expect } from "vitest";
import { renderWorkbench, startFen, summary } from "./harness";

describe("search mask", () => {
  it("filters the whole database and resets", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.keyboard("{Control>}f{/Control}");
    const mask = ui.getByRole("dialog", { name: "Search mask" });
    expect(mask).toBeVisible();
    await ui.user.type(ui.getByLabelText("White last name"), "Kasparov");
    await ui.user.click(ui.getByLabelText("Ignore colours"));
    await ui.user.type(ui.getByLabelText("Year from"), "1985");
    await ui.user.type(ui.getByLabelText("Elo difference at most"), "150");
    await ui.user.click(ui.getByLabelText("½-½"));
    await ui.user.click(ui.getByLabelText("Mate"));
    await ui.user.type(ui.getByLabelText("Full Text"), "novelty");
    await ui.user.click(ui.getByLabelText("NOT"));
    expect(ui.getByText(/this search is slower/)).toBeVisible();
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(ui.api.searchGames).toHaveBeenCalledWith(
      "db-1",
      expect.objectContaining({
        white: { last: "Kasparov", first: "" },
        ignoreColors: true,
        yearMin: 1985,
        eloDifference: 150,
        results: ["1/2-1/2"],
        mate: true,
        fullText: "novelty",
        fullTextNot: true,
      }),
      expect.any(String),
      expect.any(Function),
    );
    expect(await ui.findByText("2 of 401 entries found")).toBeVisible();
    expect(
      await ui.findByRole("button", { name: "Load entry 42" }),
    ).toBeVisible();
    expect(ui.api.readSearchPage).toHaveBeenCalledWith(
      "db-1",
      "1",
      100,
      expect.any(String),
    );
    // The mask reopens with the current filter.
    await ui.user.keyboard("{Control>}f{/Control}");
    expect(ui.getByLabelText("White last name")).toHaveValue("Kasparov");
    await ui.user.click(ui.getByRole("button", { name: "Cancel" }));

    await ui.user.click(ui.getByRole("button", { name: "Reset filter" }));
    expect(ui.api.clearSearch).toHaveBeenCalledWith("db-1");
    expect((await ui.findAllByText("401 entries"))[0]).toBeVisible();
  });
});

describe("search progress", () => {
  it("shows progress and cancels from the status bar", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    let finish: (v: unknown) => void = () => {};
    ui.api.searchGames.mockImplementationOnce(
      (
        _db: string,
        _f: unknown,
        requestId: string,
        onProgress: (p: { done: string; total: string }) => void,
      ) => {
        onProgress({ done: "100", total: "400" });
        return new Promise((resolve) => {
          finish = () =>
            resolve({
              requestId,
              hits: "0",
              scanned: "100",
              truncated: false,
              cancelled: true,
            });
        });
      },
    );
    await ui.user.keyboard("{Control>}f{/Control}");
    await ui.user.click(ui.getByLabelText("Mate"));
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(await ui.findByText(/Searching… 25%/)).toBeVisible();
    await ui.user.click(ui.getByRole("button", { name: "Cancel" }));
    expect(ui.api.cancelSearch).toHaveBeenCalledWith("db-1");
    finish(undefined);
    expect(await ui.findByText("Search cancelled")).toBeVisible();
    expect(ui.queryByRole("button", { name: "Cancel" })).toBeNull();
  });
});

describe("position search", () => {
  it("searches the position tab's boards as a pattern or a position", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.keyboard("{Control>}f{/Control}");
    await ui.user.click(ui.getByRole("tab", { name: "Position" }));
    // The current board comes in as an exact position.
    const exact = ui.getByLabelText(/Exact position/);
    expect(exact).toBeChecked();
    expect(ui.getByRole("gridcell", { name: "e1: White king" })).toBeVisible();
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(ui.api.searchPosition).toHaveBeenLastCalledWith(
      "db-1",
      expect.objectContaining({
        exact: true,
        whiteToMove: true,
        lookFor: expect.arrayContaining([
          ["e1", "K"],
          ["e8", "k"],
        ]),
        or: [],
        exclude: [],
      }),
      expect.any(String),
      expect.any(Function),
    );
    expect(await ui.findByText("2 of 401 entries found")).toBeVisible();

    // A pattern: a white knight on f5 and nothing on d5, mirrored.
    await ui.user.keyboard("{Control>}f{/Control}");
    await ui.user.click(ui.getByRole("tab", { name: "Position" }));
    await ui.user.click(ui.getByLabelText(/Exact position/));
    await ui.user.click(ui.getByRole("button", { name: "Reset" }));
    await ui.user.click(ui.getByRole("radio", { name: "White knight" }));
    await ui.user.click(ui.getByRole("gridcell", { name: "f5" }));
    await ui.user.click(ui.getByLabelText("'Exclude' board"));
    await ui.user.click(ui.getByRole("radio", { name: "White piece (any)" }));
    await ui.user.click(ui.getByRole("gridcell", { name: "d5" }));
    await ui.user.click(ui.getByRole("radio", { name: "Black piece (any)" }));
    await ui.user.click(
      ui.getByRole("gridcell", { name: "d5: White piece (any)" }),
    );
    expect(
      ui.getByRole("gridcell", {
        name: "d5: White piece (any), Black piece (any)",
      }),
    ).toBeVisible();
    await ui.user.click(ui.getByLabelText("Horizontal"));
    await ui.user.type(ui.getByLabelText("Length"), "3");
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(ui.api.searchPosition).toHaveBeenLastCalledWith(
      "db-1",
      expect.objectContaining({
        exact: false,
        lookFor: [["f5", "N"]],
        exclude: [
          ["d5", "X"],
          ["d5", "x"],
        ],
        horizontal: true,
        length: 3,
      }),
      expect.any(String),
      expect.any(Function),
    );
    // The mask reopens on the position tab with the same boards.
    await ui.findByText("2 of 401 entries found");
    await ui.user.keyboard("{Control>}f{/Control}");
    expect(ui.getByRole("tab", { name: "Position" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(
      ui.getByRole("gridcell", { name: "f5: White knight" }),
    ).toBeVisible();
  });

  it("finds the board position and opens games at it", async () => {
    const ui = renderWorkbench();
    ui.api.readSearchPage.mockImplementation(
      async (_db: string, _s: string, _l: number, requestId: string) => ({
        requestId,
        entries: [
          { ...summary, ordinal: "7", foundNode: 2 },
          { ...summary, ordinal: "42", foundNode: 4 },
        ],
      }),
    );
    await ui.user.click(ui.getByRole("button", { name: "Hard Disk" }));
    expect(ui.api.findPosition).toHaveBeenCalledWith(
      "db-1",
      expect.stringContaining("/"),
      expect.any(String),
      expect.any(Function),
    );
    expect(await ui.findByText("2 of 401 entries found")).toBeVisible();
    await ui.user.click(ui.getByRole("button", { name: "Load entry 42" }));
    expect(ui.api.loadEntry).toHaveBeenLastCalledWith(
      "db-1",
      "42",
      expect.any(String),
    );
    await ui.findByText(/Entry 42 ·/);
    expect(ui.api.positionAt).toHaveBeenLastCalledWith(
      expect.any(String),
      4,
      expect.any(String),
    );
  });

  it("creates and deletes the search booster", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(ui.getByRole("tab", { name: "Maintenance" }));
    await ui.user.click(
      ui.getByRole("button", { name: "Create Search Booster" }),
    );
    expect(await ui.findByText("Search booster is complete")).toBeVisible();
    await ui.user.click(
      ui.getByRole("button", { name: "Delete Search Booster" }),
    );
    expect(ui.api.deleteSearchBooster).toHaveBeenCalledWith("db-1");
  });
});
