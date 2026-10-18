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
