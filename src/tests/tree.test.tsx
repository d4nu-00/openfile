import { describe, it, expect } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import { BookPane } from "../components/BookPane";
import { renderWorkbench } from "./harness";

describe("opening book", () => {
  it("describes an empty preparation as a preparation", () => {
    render(
      <BookPane
        book={{
          loading: false,
          tree: {
            requestId: "p",
            status: "tree",
            games: 0,
            ended: 0,
            moves: [],
          },
        }}
        hasDatabase
        hasLine
        onPlay={() => {}}
        prepared="Against White"
        scope="search"
      />,
    );
    expect(
      screen.getByText("No game in this preparation reaches this position."),
    ).toBeVisible();
  });

  it("shows moves from the current search and can return to the database tree", async () => {
    const ui = renderWorkbench({
      search: { hits: "3", truncated: false, label: "Selected games" },
    });
    await ui.user.click(ui.getByRole("tab", { name: "Book" }));
    const book = await ui.findByLabelText("Opening book");
    await ui.user.click(
      within(book).getByRole("button", { name: "Search results" }),
    );
    expect(
      await within(book).findByText(/2 games.*Search results/),
    ).toBeVisible();
    expect(within(book).getByText("3 of 3 search hits scanned")).toBeVisible();
    await ui.user.click(
      within(book).getByRole("button", { name: "Database tree" }),
    );
    expect(
      await within(book).findByText(/10 games.*Opening tree/),
    ).toBeVisible();
  });

  it("labels a sampled and truncated search result", async () => {
    const ui = renderWorkbench({
      search: { hits: "60000", truncated: true, label: "Large search" },
    });
    ui.api.filteredOpeningTree.mockImplementation(
      async (_db, _line, requestId) => ({
        requestId,
        status: "filtered",
        games: 42,
        ended: 0,
        sampledHits: 50000,
        totalHits: 60000,
        moves: [],
      }),
    );
    await ui.user.click(ui.getByRole("tab", { name: "Book" }));
    const book = await ui.findByLabelText("Opening book");
    await ui.user.click(
      within(book).getByRole("button", { name: "Search results" }),
    );
    expect(
      await within(book).findByText("50,000 of 60,000 search hits scanned"),
    ).toBeVisible();
    expect(within(book).getByText(/Search stopped at its limit/)).toBeVisible();
  });

  it("does not show an old filtered answer after returning to the database tree", async () => {
    const ui = renderWorkbench({ search: { hits: "3", truncated: false } });
    let finish!: (value: unknown) => void;
    ui.api.filteredOpeningTree.mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    await ui.user.click(ui.getByRole("tab", { name: "Book" }));
    const book = await ui.findByLabelText("Opening book");
    await ui.user.click(
      within(book).getByRole("button", { name: "Search results" }),
    );
    await waitFor(() => expect(ui.api.filteredOpeningTree).toHaveBeenCalled());
    const requestId = ui.api.filteredOpeningTree.mock.calls.at(-1)![2];
    await ui.user.click(
      within(book).getByRole("button", { name: "Database tree" }),
    );
    expect(ui.api.cancelFilteredOpeningTree).toHaveBeenCalledWith(
      "db-1",
      requestId,
    );
    expect(
      await within(book).findByText(/10 games.*Opening tree/),
    ).toBeVisible();
    finish({ requestId, status: "filtered", games: 99, ended: 0, moves: [] });
    await waitFor(() =>
      expect(within(book).queryByText(/99 games/)).toBeNull(),
    );
  });

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
  it("shows full and filtered sample sizes and compares a chosen player's SAN moves", async () => {
    const ui = renderWorkbench();
    await ui.user.click(ui.getByRole("tab", { name: "Reference" }));
    const pane = await ui.findByLabelText("Reference search");
    const comparison = await within(pane).findByLabelText(
      "Reference move comparison",
    );
    expect(within(comparison).getByText("e4")).toBeVisible();
    expect(within(comparison).getByText("6 · 60.0%")).toBeVisible();
    expect(within(comparison).getByText("4 · 57.1%")).toBeVisible();
    expect(
      within(pane).getByText(/1 missing a rating · 2 missing a year/),
    ).toBeVisible();
    await ui.user.type(
      within(pane).getByLabelText("Minimum Elo for both players"),
      "2200",
    );
    await ui.user.type(within(pane).getByLabelText("First year"), "2020");
    await ui.user.click(within(pane).getByLabelText("Good games"));
    await waitFor(() =>
      expect(ui.api.referenceStatistics).toHaveBeenLastCalledWith(
        "db-1",
        expect.anything(),
        expect.any(String),
        expect.objectContaining({
          eloMin: 2200,
          yearMin: 2020,
          goodGames: true,
        }),
      ),
    );
    expect(await within(pane).findByText("Elo kept 8 / 10")).toBeVisible();
    expect(within(pane).getByText("Year kept 7 / 8")).toBeVisible();
    expect(within(pane).getByText("Good games kept 7 / 7")).toBeVisible();
    await ui.user.type(
      within(pane).getByLabelText("Find comparison player"),
      "Player 120",
    );
    await ui.user.click(
      await within(pane).findByRole("option", { name: /Player 120, A/ }),
    );
    await ui.user.click(within(pane).getByLabelText("Black"));
    await waitFor(() =>
      expect(ui.api.referenceStatistics).toHaveBeenLastCalledWith(
        "db-1",
        expect.anything(),
        expect.any(String),
        expect.objectContaining({ playerId: 120, playerBlack: true }),
      ),
    );
    expect(within(pane).getByText("Player 120, A as Black: 2")).toBeVisible();
    expect(within(pane).getByText(/both players Elo ≥ 2200/)).toBeVisible();
    await ui.user.keyboard("{Control>}{Shift>}{F7}{/Shift}{/Control}");
    const extra = await ui.findByLabelText("Reference pane");
    expect(
      within(extra).getByRole("textbox", { name: "Find comparison player" }),
    ).toHaveValue("Player 120, A");
  });
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

  it("asks before reading every game of a large database", async () => {
    const ui = renderWorkbench();
    ui.api.referenceSearch.mockImplementation(
      async (
        _db: string,
        _line: unknown,
        requestId: string,
        searchAnyway?: boolean,
      ) => ({
        requestId,
        hits: "0",
        truncated: false,
        cancelled: false,
        source: searchAnyway ? "search" : "notSearched",
        indexed: false,
      }),
    );
    await ui.user.click(ui.getByRole("tab", { name: "Reference" }));
    const pane = await ui.findByLabelText("Reference search");
    expect(
      await within(pane).findByText(/not in the opening tree/),
    ).toBeVisible();
    await ui.user.click(
      within(pane).getByRole("button", { name: "Search all games" }),
    );
    await waitFor(() =>
      expect(ui.api.referenceSearch).toHaveBeenLastCalledWith(
        "db-1",
        expect.anything(),
        expect.any(String),
        true,
      ),
    );
    expect(
      await within(pane).findByText(/0 games by position search/),
    ).toBeVisible();
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
