import type { SrsGrade, SrsOptions, SrsStudyCard } from "../api/types";

/** The opponent's moves are played after this long, so they can be seen. */
export const OPPONENT_DELAY_MS = 400;

/** The time the scheduler works from: now, and the local offset in minutes. */
export function clock() {
  return { now: Date.now(), utcOffset: -new Date().getTimezoneOffset() };
}

export const defaultOptions: SrsOptions = {
  learningSteps: [1, 10],
  relearningSteps: [10],
  graduatingInterval: 1,
  easyInterval: 4,
  newPerDay: 20,
  reviewsPerDay: 200,
  maxInterval: 36500,
  newOrder: "source",
  includeVariations: true,
  trainee: "sideToMove",
};

/** Steps as typed: minutes separated by commas or spaces. */
export function parseSteps(text: string): number[] | undefined {
  const parts = text.split(/[\s,]+/).filter(Boolean);
  const steps = parts.map(Number);
  return steps.every((s) => Number.isFinite(s) && s > 0) ? steps : undefined;
}
export const formatSteps = (steps: number[]) => steps.join(", ");

/** The same checks as the backend; the first problem, if any. */
export function validateOptions(o: SrsOptions): string | undefined {
  const stepsOk = (steps: number[]) =>
    steps.length <= 20 && steps.every((s) => s > 0 && s <= 43200);
  if (o.learningSteps.length === 0 || !stepsOk(o.learningSteps))
    return "Learning steps are 1 to 20 times in minutes, up to 30 days.";
  if (!stepsOk(o.relearningSteps))
    return "Relearning steps are up to 20 times in minutes, up to 30 days.";
  const days = (v: number) => v >= 1 && v <= 36500;
  if (!days(o.graduatingInterval) || !days(o.easyInterval))
    return "Intervals are from 1 to 36500 days.";
  if (o.easyInterval < o.graduatingInterval)
    return "The easy interval is not shorter than the graduating interval.";
  if (!days(o.maxInterval))
    return "The maximum interval is from 1 to 36500 days.";
  const limit = (v: number) => Number.isInteger(v) && v >= 0 && v <= 100000;
  if (!limit(o.newPerDay) || !limit(o.reviewsPerDay))
    return "New cards and reviews per day are whole numbers from 0 to 100000.";
  return undefined;
}

/** How long until a card is due, as shown under a grade button. */
export function intervalLabel(ms: number): string {
  const trim = (n: number) => String(Math.round(n * 10) / 10);
  const minutes = ms / 60_000;
  if (minutes < 1) return "<1m";
  if (minutes < 60) return `${Math.round(minutes)}m`;
  const hours = minutes / 60;
  if (hours < 24) return `${trim(hours)}h`;
  const days = hours / 24;
  if (days < 30) return `${trim(days)}d`;
  if (days < 365) return `${trim(days / 30)}mo`;
  return `${trim(days / 365)}y`;
}

/** A mistake always ends in Again; a hint caps the grade at Hard. */
export function effectiveGrade(
  chosen: SrsGrade,
  mistakes: number,
  hints: number,
): SrsGrade {
  if (mistakes > 0) return "again";
  if (hints > 0 && (chosen === "good" || chosen === "easy")) return "hard";
  return chosen;
}

/** How far a card has been played. */
export interface SrsRun {
  /** The index of the next move. */
  ply: number;
  mistakes: number;
  hints: number;
  /** What the last attempt showed: a move of another line, a mistake, or
   * a wrong move on a new card. */
  note?: "other" | "mistake" | "learn";
  /** A move shown as an arrow after a hint or a mistake. */
  arrow?: string;
}
export const startRun = (): SrsRun => ({ ply: 0, mistakes: 0, hints: 0 });

export const finished = (study: SrsStudyCard, run: SrsRun) =>
  run.ply >= study.card.moves.length;

/** Whether the trainee moves at `ply`: the side to move there is theirs. */
export function isTraineePly(study: SrsStudyCard, ply: number): boolean {
  const fen = study.plies[ply]?.fen;
  if (!fen) return false;
  return (fen.split(" ")[1] === "w") === (study.card.trainee === "white");
}

/** The opponent's move was played. */
export const opponentPlays = (run: SrsRun): SrsRun => ({
  ...run,
  ply: run.ply + 1,
  note: undefined,
  arrow: undefined,
});

/** The trainee plays `uci` (a legal move) at the current ply. */
export function tryMove(study: SrsStudyCard, run: SrsRun, uci: string): SrsRun {
  const move = study.card.moves[run.ply];
  if (!move || !isTraineePly(study, run.ply)) return run;
  if (uci === move.uci)
    return { ...run, ply: run.ply + 1, note: undefined, arrow: undefined };
  // A new card is a lesson: a wrong move just shows the arrow again.
  if (study.learn) return { ...run, note: "learn", arrow: move.uci };
  if (study.plies[run.ply].otherMoves.includes(uci))
    return { ...run, note: "other" };
  return {
    ...run,
    mistakes: run.mistakes + 1,
    note: "mistake",
    arrow: move.uci,
  };
}

export function giveHint(study: SrsStudyCard, run: SrsRun): SrsRun {
  const move = study.card.moves[run.ply];
  if (!move || !isTraineePly(study, run.ply)) return run;
  return { ...run, hints: run.hints + 1, arrow: move.uci };
}

/** The arrow to draw: always on a new card, else after a hint or mistake. */
export function arrowOf(
  study: SrsStudyCard,
  run: SrsRun,
): { from: string; to: string } | undefined {
  const move = study.card.moves[run.ply];
  if (!move || !isTraineePly(study, run.ply)) return undefined;
  const uci = study.learn ? move.uci : run.arrow;
  return uci ? { from: uci.slice(0, 2), to: uci.slice(2, 4) } : undefined;
}
