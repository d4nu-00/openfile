/**
 * The selected rows of a list, as sorted, disjoint ranges [start, end) of
 * row indexes, so selecting all of millions of rows stays small.
 */
export interface Selection {
  ranges: [number, number][];
  /** The row a Shift-click extends from. */
  anchor?: number;
}

export const noSelection: Selection = { ranges: [] };

export const isSelected = (s: Selection, row: number) =>
  s.ranges.some(([a, b]) => row >= a && row < b);

export const selectedCount = (s: Selection) =>
  s.ranges.reduce((n, [a, b]) => n + (b - a), 0);

function normalise(ranges: [number, number][]): [number, number][] {
  const sorted = ranges.filter(([a, b]) => b > a).sort((x, y) => x[0] - y[0]);
  const out: [number, number][] = [];
  for (const [a, b] of sorted) {
    const last = out.at(-1);
    if (last && a <= last[1]) last[1] = Math.max(last[1], b);
    else out.push([a, b]);
  }
  return out;
}

/** The selection after clicking `row`: alone, toggled (Ctrl) or extended
 * from the anchor (Shift). */
export function chooseRow(
  s: Selection,
  row: number,
  mods: { toggle?: boolean; extend?: boolean } = {},
): Selection {
  if (mods.extend && s.anchor !== undefined) {
    const [a, b] = [Math.min(s.anchor, row), Math.max(s.anchor, row) + 1];
    const base = mods.toggle ? s.ranges : [];
    return { anchor: s.anchor, ranges: normalise([...base, [a, b]]) };
  }
  if (mods.toggle) {
    if (!isSelected(s, row))
      return { anchor: row, ranges: normalise([...s.ranges, [row, row + 1]]) };
    const ranges: [number, number][] = [];
    for (const [a, b] of s.ranges) {
      if (row < a || row >= b) ranges.push([a, b]);
      else {
        ranges.push([a, row], [row + 1, b]);
      }
    }
    return { anchor: row, ranges: normalise(ranges) };
  }
  return { anchor: row, ranges: [[row, row + 1]] };
}

export const selectAll = (rows: number): Selection => ({
  anchor: 0,
  ranges: rows > 0 ? [[0, rows]] : [],
});

/** The selection as backend row ranges, counted from 1. */
export const rowRanges = (s: Selection) =>
  s.ranges.map(([a, b]) => ({ start: String(a + 1), end: String(b + 1) }));
