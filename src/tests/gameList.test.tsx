import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { renderWorkbench, summary } from "./harness";
import { RowCache } from "../state/gameList";
import { nextSort } from "../views/GameList";

describe("virtual game list", () => {
  it("scrolls with the wheel and reads rows a block at a time", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await screen.findByRole("button", { name: "Load entry 1" });
    expect(ui.api.readRows).toHaveBeenCalledWith(
      "db-1",
      "1",
      100,
      expect.any(String),
    );
    const list = screen.getByLabelText("Game list");
    fireEvent.wheel(list, { deltaY: 220 });
    expect(
      await screen.findByRole("button", { name: "Load entry 11" }),
    ).toBeVisible();
    expect(screen.queryByRole("button", { name: "Load entry 1" })).toBeNull();
    expect(
      screen.getByRole("scrollbar", { name: "Game list position" }),
    ).toHaveAttribute("aria-valuenow", "11");
    // Far down the list, another block is read.
    fireEvent.wheel(list, { deltaY: 22 * 300 });
    expect(
      await screen.findByRole("button", { name: "Load entry 311" }),
    ).toBeVisible();
    expect(ui.api.readRows).toHaveBeenCalledWith(
      "db-1",
      "301",
      100,
      expect.any(String),
    );
  });

  it("goes to an entry's row and keeps it in view", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    const input = screen.getByLabelText("Entry number");
    await ui.user.clear(input);
    await ui.user.type(input, "250");
    await ui.user.click(screen.getByRole("button", { name: "Go to entry" }));
    expect(ui.api.findRow).toHaveBeenCalledWith("db-1", "250");
    const button = await screen.findByRole("button", {
      name: "Load entry 250",
    });
    expect(button.closest("tr")).toHaveAttribute("aria-selected", "true");
    await waitFor(() =>
      expect(ui.api.loadEntry).toHaveBeenLastCalledWith(
        "db-1",
        "250",
        expect.any(String),
      ),
    );
  });

  it("moves through rows with the keyboard", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(
      await screen.findByRole("button", { name: "Load entry 3" }),
    );
    screen.getByLabelText("Game list").focus();
    await ui.user.keyboard("{ArrowDown}");
    await waitFor(() =>
      expect(ui.api.loadEntry).toHaveBeenLastCalledWith(
        "db-1",
        "4",
        expect.any(String),
      ),
    );
  });
});

describe("sorting", () => {
  it("cycles a column through ascending, descending and entry order", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await screen.findByRole("button", { name: "Load entry 1" });
    const header = () =>
      screen.getByRole("button", { name: "Sort by White" }).closest("th")!;
    await ui.user.click(screen.getByRole("button", { name: "Sort by White" }));
    expect(ui.api.sortList).toHaveBeenLastCalledWith(
      "db-1",
      "white",
      false,
      expect.any(Function),
    );
    await waitFor(() =>
      expect(header()).toHaveAttribute("aria-sort", "ascending"),
    );
    await ui.user.click(screen.getByRole("button", { name: "Sort by White" }));
    expect(ui.api.sortList).toHaveBeenLastCalledWith(
      "db-1",
      "white",
      true,
      expect.any(Function),
    );
    await waitFor(() =>
      expect(header()).toHaveAttribute("aria-sort", "descending"),
    );
    await ui.user.click(screen.getByRole("button", { name: "Sort by White" }));
    expect(ui.api.sortList).toHaveBeenLastCalledWith(
      "db-1",
      null,
      false,
      expect.any(Function),
    );
    await waitFor(() => expect(header()).toHaveAttribute("aria-sort", "none"));
    expect(await screen.findByText("Entry order")).toBeVisible();
  });

  it("reports a cancelled sort and keeps the order", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    ui.api.sortList.mockImplementationOnce(async () => ({
      rows: "401",
      cancelled: true,
      descending: false,
    }));
    await ui.user.click(
      await screen.findByRole("button", { name: "Sort by Date" }),
    );
    expect(await screen.findByText("Sorting cancelled")).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Sort by Date" }).closest("th"),
    ).toHaveAttribute("aria-sort", "none");
  });

  it("computes the next sort of a header", () => {
    expect(nextSort(undefined, "date")).toEqual({
      column: "date",
      descending: false,
    });
    expect(nextSort({ column: "date", descending: false }, "date")).toEqual({
      column: "date",
      descending: true,
    });
    expect(nextSort({ column: "date", descending: true }, "date")).toBe(
      undefined,
    );
    expect(nextSort({ column: "date", descending: true }, "eco")).toEqual({
      column: "eco",
      descending: false,
    });
  });
});

describe("list and search", () => {
  it("quick filter searches the whole database", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(screen.getByRole("button", { name: "Quick Filter" }));
    await ui.user.type(screen.getByLabelText("Filter games"), "Kasparov 1985");
    await ui.user.keyboard("{Enter}");
    expect(ui.api.searchGames).toHaveBeenCalledWith(
      "db-1",
      expect.objectContaining({ text: "Kasparov 1985" }),
      expect.any(String),
    );
    expect(await screen.findByText("2 of 401 entries found")).toBeVisible();
  });

  it("loads the next game of the list, not the next entry", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.keyboard("{Control>}f{/Control}");
    await ui.user.click(
      within(screen.getByRole("dialog", { name: "Search mask" })).getByRole(
        "button",
        { name: "Search" },
      ),
    );
    await ui.user.click(
      await screen.findByRole("button", { name: "Load entry 7" }),
    );
    await screen.findByText(/Entry 7 ·/);
    await ui.user.click(screen.getByRole("button", { name: /Open game/ }));
    await ui.user.keyboard("{F11}");
    await waitFor(() =>
      expect(ui.api.loadEntry).toHaveBeenLastCalledWith(
        "db-1",
        "42",
        expect.any(String),
      ),
    );
  });
});

describe("row cache", () => {
  it("ignores rows of an older list and bounds its blocks", async () => {
    let release: (() => void) | undefined;
    const readRows = vi.fn(
      (_db: string, start: string, limit: number, _requestId: string) =>
        new Promise<(typeof summary)[]>((resolve) => {
          const answer = () =>
            resolve(
              Array.from({ length: limit }, (_, i) => ({
                ...summary,
                ordinal: String(Number(start) + i),
              })),
            );
          if (start === "1" && !release) release = answer;
          else answer();
        }),
    );
    const changed = vi.fn();
    const cache = new RowCache(readRows, changed);
    cache.reset("a", "db-1");
    cache.want(0, 10, 1000);
    // A new list starts before the old answer arrives.
    cache.reset("b", "db-1");
    release!();
    await Promise.resolve();
    expect(cache.row(0)).toBeUndefined();
    expect(await cache.entry(5, 1000)).toMatchObject({ ordinal: "6" });
    for (let b = 0; b < 70; b++) await cache.entry(b * 100, 10_000);
    // The first blocks were dropped; the last ones are kept.
    expect(cache.row(0)).toBeUndefined();
    expect(cache.row(6900)).toMatchObject({ ordinal: "6901" });
    expect(cache.find((r) => r.ordinal === "6901")).toBe(6900);
  });
});
