import { describe, it, expect } from "vitest";
import { renderWorkbench, summary } from "./harness";

describe("search mask", () => {
  it("filters the whole database and resets", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.keyboard("{Control>}f{/Control}");
    const mask = ui.getByRole("dialog", { name: "Search mask" });
    expect(mask).toBeVisible();
    await ui.user.type(ui.getByLabelText("White last name"), "Kasparov");
    await ui.user.click(ui.getByLabelText("Ignore colours"));
    await ui.user.type(ui.getByLabelText("Year from"), "1985");
    await ui.user.click(ui.getByLabelText("½-½"));
    await ui.user.click(ui.getByRole("button", { name: "Search" }));
    expect(ui.api.searchGames).toHaveBeenCalledWith(
      "db-1",
      expect.objectContaining({
        white: { last: "Kasparov", first: "" },
        ignoreColors: true,
        yearMin: 1985,
        results: ["1/2-1/2"],
      }),
      expect.any(String),
    );
    expect(await ui.findByText("2 of 401 entries found")).toBeVisible();
    expect(ui.getByRole("button", { name: "Load entry 42" })).toBeVisible();
    expect(ui.api.readSearchPage).toHaveBeenCalledWith(
      "db-1",
      "1",
      200,
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

describe("position search", () => {
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
