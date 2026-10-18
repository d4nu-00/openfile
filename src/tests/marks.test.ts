import { it, expect } from "vitest";
import { toggleMark } from "../state/marks";

it("adds, recolours and removes marks like the reference app", () => {
  const empty = { squares: [], arrows: [] };
  const green = toggleMark(empty, { from: "e4", to: "e4", color: "G" });
  expect(green.squares).toEqual([{ color: "G", square: "e4" }]);
  const red = toggleMark(green, { from: "e4", to: "e4", color: "R" });
  expect(red.squares).toEqual([{ color: "R", square: "e4" }]);
  expect(toggleMark(red, { from: "e4", to: "e4", color: "R" }).squares).toEqual(
    [],
  );
  const arrow = toggleMark(empty, { from: "g1", to: "f3", color: "G" });
  expect(arrow.arrows).toEqual([{ color: "G", from: "g1", to: "f3" }]);
  expect(
    toggleMark(arrow, { from: "g1", to: "f3", color: "G" }).arrows,
  ).toEqual([]);
});
