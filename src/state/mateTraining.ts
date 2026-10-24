/**
 * Combinations of pieces in a mate, as Build Mate Training names them: Q, R,
 * B, N, P and K in that order joined by "+", with m for a bishop or knight
 * when they are pooled. The same rules as the backend's
 * `normalise_combination`.
 */
export function normaliseCombination(
  text: string,
  poolMinors: boolean,
): string | null {
  const have = new Set<number>();
  for (const part of text.split("+")) {
    const piece = part.trim().toUpperCase();
    const order = poolMinors
      ? { Q: 0, R: 1, M: 2, B: 2, N: 2, P: 4, K: 5 }[piece]
      : { Q: 0, R: 1, B: 2, N: 3, P: 4, K: 5 }[piece];
    if (order === undefined) return null;
    have.add(order);
  }
  const names = ["Q", "R", poolMinors ? "m" : "B", "N", "P", "K"];
  return [...have]
    .sort((a, b) => a - b)
    .map((i) => names[i])
    .join("+");
}

/**
 * The combinations in a comma-separated list, or the first entry that is not
 * one.
 */
export function parseCombinations(
  list: string,
  poolMinors: boolean,
): { combinations: string[] } | { invalid: string } {
  const combinations: string[] = [];
  for (const entry of list.split(",")) {
    if (!entry.trim()) continue;
    const normal = normaliseCombination(entry, poolMinors);
    if (normal === null) return { invalid: entry.trim() };
    if (!combinations.includes(normal)) combinations.push(normal);
  }
  return { combinations };
}

/** What the backend counts when it does not use a game or a step. */
export const rejectionLabels: Record<string, string> = {
  notMate: "not a forced mate",
  shorter: "a shorter mate",
  longer: "a longer mate",
  notUnique: "more than one mating move",
  otherMove: "a mate with another move",
  timeout: "no engine score in time",
  notMateGame: "game does not end in mate",
  chess960: "Chess960 game",
  unreadable: "unreadable game",
  underpromotion: "mate by underpromotion",
  duplicate: "position already collected",
};
