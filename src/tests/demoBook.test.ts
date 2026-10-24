import { expect, it } from "vitest";
import { demoApi } from "../dev/demoApi";
import { emptyFilter } from "../api/types";
import { demoFens } from "../dev/demoGame";

it("shows algebraic move names in both demo opening trees", async () => {
  const api = demoApi();
  const line = { startFen: demoFens[0], moves: [] };
  const full = await api.openingTree("demo", line, "full");
  expect(full.moves.slice(0, 3).map((move) => move.san)).toEqual([
    "e4",
    "d4",
    "Nf3",
  ]);

  await api.searchGames("demo", emptyFilter(), "search");
  const filtered = await api.filteredOpeningTree("demo", line, "filtered");
  expect(filtered.moves.map((move) => move.san)).toEqual(["e4", "d4", "Nf3"]);
});

it("previews within/add scopes using entry identities", async () => {
  const api = demoApi();
  await api.showEntries("demo", ["1", "2", "2"], "seed");
  const q = {
    lookFor: [["e4", "P"] as [string, string]],
    or: [],
    exclude: [],
    exact: false,
    horizontal: false,
    vertical: false,
    lines: false,
  };
  const within = await api.searchPosition(
    "demo",
    q,
    "within",
    undefined,
    "within",
  );
  expect(within.hits).toBe("1");
  const added = await api.searchGames(
    "demo",
    emptyFilter(),
    "add",
    undefined,
    "add",
  );
  expect(added.hits).toBe("200");
  const page = await api.readSearchPage("demo", "1", 10, "page");
  expect(page.next).toBe("11");
  const second = await api.readSearchPage("demo", "11", 10, "page2");
  expect(second.entries[0].ordinal).toBe("121");
  expect(page.entries.slice(0, 3).map((e) => e.ordinal)).toEqual([
    "1",
    "13",
    "25",
  ]);
});
