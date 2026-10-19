import type { GameView, ReviewedMove } from "../api/types";

export type PracticeResult = "solved" | "needsPractice";
export type PracticeProgress = Record<string, PracticeResult>;

/** A game and its exact main line own the practice record, not a transient tab. */
export function practiceKey(game: GameView): string {
  const line = [game.startFen];
  let node = 0;
  for (let i = 0; i < game.nodes.length; i++) {
    const next = game.nodes[node]?.children[0];
    if (next === undefined || !game.nodes[next]) break;
    line.push(game.nodes[next].uci ?? "");
    node = next;
  }
  return `openfile.reviewPractice.v1:${JSON.stringify([
    game.tags.White,
    game.tags.Black,
    game.tags.Date,
    game.tags.Event,
    game.tags.Site,
    game.tags.Round,
    ...line,
  ])}`;
}

export function practiceItemKey(move: ReviewedMove): string {
  return `${move.node}:${move.bestUci ?? ""}`;
}

export function readPractice(key: string): PracticeProgress {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(key) ?? "{}");
    if (!stored || typeof stored !== "object" || Array.isArray(stored))
      return {};
    return Object.fromEntries(
      Object.entries(stored).filter(
        ([, value]) => value === "solved" || value === "needsPractice",
      ),
    );
  } catch {
    return {};
  }
}

export function writePractice(key: string, progress: PracticeProgress): void {
  try {
    localStorage.setItem(key, JSON.stringify(progress));
  } catch {
    // Practice remains available for this session when storage is unavailable.
  }
}

export function recordPractice(
  progress: PracticeProgress,
  move: ReviewedMove,
  result: PracticeResult,
): PracticeProgress {
  const item = practiceItemKey(move);
  return { ...progress, [item]: result };
}
