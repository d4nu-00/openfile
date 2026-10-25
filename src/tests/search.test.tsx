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

describe("final material search", () => {
  it("sends equal pieces with the bishop and knight option", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.keyboard("{Control>}f{/Control}");
    const equal = ui.getByRole("checkbox", { name: "Equal pieces" });
    const minors = ui.getByRole("checkbox", { name: "Bishop = Knight" });
    const pawns = ui.getByRole("checkbox", { name: "Count pawns" });
    // The options wait for the criterion; B = N is on unless turned off.
    expect(minors).toBeDisabled();
    expect(pawns).toBeDisabled();
    expect(minors).toBeChecked();
    await ui.user.click(equal);
    expect(minors).toBeEnabled();
    expect(pawns).toBeEnabled();
    await ui.user.click(ui.getByLabelText("1-0"));
    expect(ui.getByText(/this search is slower/)).toBeVisible();
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(ui.api.searchGames).toHaveBeenLastCalledWith(
      "db-1",
      expect.objectContaining({
        results: ["1-0"],
        finalMaterial: {
          equal: true,
          separateMinors: false,
          countPawns: false,
        },
      }),
      expect.any(String),
      expect.any(Function),
    );

    await ui.user.keyboard("{Control>}f{/Control}");
    await ui.user.click(ui.getByRole("checkbox", { name: "Bishop = Knight" }));
    await ui.user.click(ui.getByRole("checkbox", { name: "Count pawns" }));
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(ui.api.searchGames).toHaveBeenLastCalledWith(
      "db-1",
      expect.objectContaining({
        finalMaterial: { equal: true, separateMinors: true, countPawns: true },
      }),
      expect.any(String),
      expect.any(Function),
    );

    // Reset all clears the criterion.
    await ui.user.keyboard("{Control>}f{/Control}");
    await ui.user.click(ui.getByRole("button", { name: "Reset all" }));
    expect(
      ui.getByRole("checkbox", { name: "Equal pieces" }),
    ).not.toBeChecked();
    expect(ui.getByRole("checkbox", { name: "Bishop = Knight" })).toBeChecked();
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(ui.api.searchGames).toHaveBeenLastCalledWith(
      "db-1",
      expect.not.objectContaining({
        finalMaterial: expect.objectContaining({ equal: true }),
      }),
      expect.any(String),
      expect.any(Function),
    );
  });
});

describe("ending tab", () => {
  const any = {};
  const counts = (patch = {}) => ({
    kings: any,
    queens: any,
    rooks: any,
    bishops: any,
    knights: any,
    minors: any,
    pawns: any,
    othersZero: false,
    ...patch,
  });
  const openEnding = async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.keyboard("{Control>}f{/Control}");
    await ui.user.click(ui.getByRole("tab", { name: "Ending" }));
    return ui;
  };

  it("sends the pieces involved in a mate", async () => {
    const ui = await openEnding();
    // The selects wait for the checkbox.
    expect(ui.getByLabelText("Mating queens")).toBeDisabled();
    await ui.user.click(
      ui.getByRole("checkbox", { name: "Final position is mate by" }),
    );
    expect(ui.getByLabelText("Mating queens")).toBeEnabled();
    await ui.user.selectOptions(ui.getByLabelText("Mating queens"), "1");
    await ui.user.selectOptions(ui.getByLabelText("Mating minor pieces"), "1");
    await ui.user.click(
      ui.getByRole("checkbox", { name: "Mate by: no other pieces" }),
    );
    expect(ui.getByText(/this search is slower/)).toBeVisible();
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(ui.api.searchGames).toHaveBeenLastCalledWith(
      "db-1",
      expect.objectContaining({
        ending: {
          white: counts(),
          black: counts(),
          eitherColour: false,
          mateByEnabled: true,
          mateBy: counts({
            queens: { min: 1, max: 1 },
            minors: { min: 1, max: 1 },
            othersZero: true,
          }),
        },
      }),
      expect.any(String),
      expect.any(Function),
    );
  });

  it("shows bishops and knights apart when Bishop = Knight is off", async () => {
    const ui = await openEnding();
    expect(ui.getByLabelText("White minor pieces")).toBeVisible();
    expect(ui.getByLabelText("Mating minor pieces")).toBeVisible();
    expect(ui.queryByLabelText("Mating bishops")).toBeNull();
    await ui.user.selectOptions(ui.getByLabelText("White minor pieces"), "2");
    await ui.user.click(ui.getByRole("checkbox", { name: "Bishop = Knight" }));
    expect(ui.getByLabelText("Mating bishops")).toBeVisible();
    expect(ui.getByLabelText("Mating knights")).toBeVisible();
    expect(ui.getByLabelText("White bishops")).toBeVisible();
    expect(ui.getByLabelText("Black knights")).toBeVisible();
    expect(ui.queryByLabelText("White minor pieces")).toBeNull();
    expect(ui.queryByLabelText("Mating minor pieces")).toBeNull();
    // The hidden Minor column no longer counts.
    await ui.user.selectOptions(ui.getByLabelText("White bishops"), "1+");
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(ui.api.searchGames).toHaveBeenLastCalledWith(
      "db-1",
      expect.objectContaining({
        ending: expect.objectContaining({
          white: counts({ bishops: { min: 1 } }),
        }),
      }),
      expect.any(String),
      expect.any(Function),
    );
  });

  it("sends final piece counts for either colour and resets", async () => {
    const ui = await openEnding();
    await ui.user.selectOptions(ui.getByLabelText("White rooks"), "1+");
    await ui.user.selectOptions(ui.getByLabelText("Black pawns"), "8");
    await ui.user.click(ui.getByRole("checkbox", { name: "Either colour" }));
    await ui.user.click(ui.getByRole("checkbox", { name: "No other pieces" }));
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(ui.api.searchGames).toHaveBeenLastCalledWith(
      "db-1",
      expect.objectContaining({
        ending: {
          white: counts({ rooks: { min: 1 }, othersZero: true }),
          black: counts({ pawns: { min: 8, max: 8 }, othersZero: true }),
          eitherColour: true,
          mateByEnabled: false,
          mateBy: counts(),
        },
      }),
      expect.any(String),
      expect.any(Function),
    );

    await ui.user.keyboard("{Control>}f{/Control}");
    await ui.user.click(ui.getByRole("tab", { name: "Ending" }));
    expect(ui.getByLabelText("White rooks")).toHaveValue("1+");
    await ui.user.click(ui.getByRole("button", { name: "Reset all" }));
    expect(ui.getByLabelText("White rooks")).toHaveValue("any");
    expect(
      ui.getByRole("checkbox", { name: "Either colour" }),
    ).not.toBeChecked();
    expect(
      ui.getByRole("checkbox", { name: "No other pieces" }),
    ).not.toBeChecked();
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(ui.api.searchGames).toHaveBeenLastCalledWith(
      "db-1",
      expect.not.objectContaining({
        ending: expect.anything(),
      }),
      expect.any(String),
      expect.any(Function),
    );
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

describe("material and result scopes", () => {
  it("builds a rook ending with a signed balance and refines current results", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.keyboard("{Control>}f{/Control}");
    expect(ui.getByRole("radio", { name: "Within results" })).toBeDisabled();
    expect(ui.getByRole("radio", { name: "Add to results" })).toBeDisabled();
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    await ui.findByText("2 of 401 entries found");
    await ui.user.keyboard("{Control>}f{/Control}");
    await ui.user.click(ui.getByRole("tab", { name: "Material" }));
    await ui.user.click(ui.getByLabelText("Use material criteria"));
    await ui.user.selectOptions(ui.getByLabelText("Endgame type"), "rook");
    await ui.user.type(ui.getByLabelText("Material balance from"), "-2");
    await ui.user.type(ui.getByLabelText("Material balance to"), "2");
    await ui.user.click(ui.getByLabelText("Ignore material colours"));
    await ui.user.click(ui.getByRole("radio", { name: "Within results" }));
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(ui.api.searchGames).toHaveBeenLastCalledWith(
      "db-1",
      expect.objectContaining({
        material: expect.objectContaining({
          white: expect.objectContaining({
            queens: { max: 0 },
            rooks: { min: 1 },
            bishops: { max: 0 },
            knights: { max: 0 },
          }),
          black: expect.objectContaining({
            queens: { max: 0 },
            rooks: { min: 1 },
          }),
          balance: { min: -2, max: 2 },
          ignoreColors: true,
        }),
      }),
      expect.any(String),
      expect.any(Function),
      "within",
    );
    await ui.findByText("2 of 401 entries found");
    await ui.user.keyboard("{Control>}f{/Control}");
    await ui.user.click(ui.getByRole("tab", { name: "Material" }));
    expect(ui.getByLabelText("Material balance from")).toHaveValue(-2);
    await ui.user.click(ui.getByRole("radio", { name: "Add to results" }));
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(ui.api.searchGames.mock.calls.at(-1)?.[4]).toBe("add");
  });

  it("rejects reversed material ranges and resets every criterion and scope", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.keyboard("{Control>}f{/Control}");
    await ui.user.click(ui.getByRole("tab", { name: "Material" }));
    await ui.user.click(ui.getByLabelText("Use material criteria"));
    await ui.user.type(ui.getByLabelText("White rooks from"), "2");
    await ui.user.type(ui.getByLabelText("White rooks to"), "1");
    expect(ui.getByRole("button", { name: "Search" })).toBeDisabled();
    expect(ui.getByRole("alert")).toHaveTextContent(/range/i);
    await ui.user.click(ui.getByRole("button", { name: "Reset all" }));
    expect(ui.getByLabelText("Use material criteria")).not.toBeChecked();
    expect(ui.getByRole("radio", { name: "All games" })).toBeChecked();
    expect(ui.getByRole("button", { name: "Search" })).toBeEnabled();
  });
});

describe("material mask recovery", () => {
  it("resets the displayed preset when criteria are reset or re-enabled", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.keyboard("{Control>}f{/Control}");
    await ui.user.click(ui.getByRole("tab", { name: "Material" }));
    await ui.user.click(ui.getByLabelText("Use material criteria"));
    await ui.user.selectOptions(ui.getByLabelText("Endgame type"), "rook");
    await ui.user.click(ui.getByRole("button", { name: "Reset all" }));
    expect(ui.getByLabelText("Endgame type")).toHaveValue("any");
    await ui.user.click(ui.getByLabelText("Use material criteria"));
    await ui.user.selectOptions(ui.getByLabelText("Endgame type"), "rook");
    await ui.user.click(ui.getByLabelText("Use material criteria"));
    await ui.user.click(ui.getByLabelText("Use material criteria"));
    expect(ui.getByLabelText("Endgame type")).toHaveValue("any");
  });

  it("allows a position-only search despite invalid material on another tab", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.keyboard("{Control>}f{/Control}");
    await ui.user.click(ui.getByRole("tab", { name: "Material" }));
    await ui.user.click(ui.getByLabelText("Use material criteria"));
    await ui.user.type(ui.getByLabelText("White rooks from"), "2");
    await ui.user.type(ui.getByLabelText("White rooks to"), "1");
    await ui.user.click(ui.getByRole("tab", { name: "Position" }));
    expect(ui.getByRole("button", { name: "Search" })).toBeEnabled();
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(ui.api.searchPosition).toHaveBeenCalled();
  });
});
