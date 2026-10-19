import type {
  GameView,
  PositionResponse,
  Training,
  TrainingAnswer,
} from "../api/types";
import { shownText } from "../components/TrainingDialog";

export interface ReplayQuestion {
  key: string;
  label: string;
  node: number;
  fen: string;
  turn: "white" | "black";
  legalUci: string[];
  training: Training;
  answers: {
    value: string | number;
    points: number;
    feedback: string;
    label: string;
  }[];
  maxPoints: number;
}
export interface ReplayProgress {
  attempts: number;
  needsRetry: boolean;
  bestPoints?: number;
}
export type ReplayProgressMap = Record<string, ReplayProgress>;
export interface ReplayGrade {
  kind: "correct" | "authoredWrong" | "ungraded";
  points: number;
  feedback: string;
  maxPoints: number;
}

const square = (value: number | undefined) =>
  value !== undefined && Number.isInteger(value) && value >= 0 && value < 64
    ? `${"abcdefgh"[Math.floor(value / 8)]}${(value % 8) + 1}`
    : undefined;

export function collectQuestionNodes(game: GameView): number[] {
  const found: number[] = [],
    seen = new Set<number>();
  const visit = (id: number) => {
    if (seen.has(id) || !game.nodes[id] || game.nodes[id].id !== id) return;
    seen.add(id);
    if (id && game.nodes[id].training) found.push(id);
    for (const child of game.nodes[id].children) visit(child);
  };
  visit(0);
  return found;
}

function pathTo(game: GameView, node: number): string[] | null {
  const path: string[] = [],
    seen = new Set<number>();
  for (let id = node; id !== 0;) {
    if (seen.has(id)) return null;
    seen.add(id);
    const n = game.nodes[id];
    if (
      !n?.uci ||
      n.parent === undefined ||
      !game.nodes[n.parent]?.children.includes(id)
    )
      return null;
    path.unshift(n.uci);
    id = n.parent;
  }
  return path;
}

function answerMove(
  answer: TrainingAnswer,
  legal: string[],
): string | undefined {
  const from = square(answer.from),
    to = square(answer.to);
  if (!from || !to) return undefined;
  return legal.find(
    (uci) =>
      uci.startsWith(from + to) && (uci.length === 4 || uci.endsWith("q")),
  );
}

/** Validates the saved annotation against the current parent position. */
export function prepareQuestion(
  game: GameView,
  node: number,
  position: PositionResponse,
  label: string,
): ReplayQuestion | null {
  const entry = game.nodes[node],
    training = entry?.training;
  const path = pathTo(game, node);
  if (
    !training ||
    !path ||
    entry.parent !== position.nodeId ||
    !shownText(training.question).trim() ||
    !Array.isArray(training.answers)
  )
    return null;
  const answers = training.answers.flatMap((answer, index) => {
    const value =
      training.kind === "moveInput"
        ? answerMove(answer, position.legalUci)
        : index;
    const choice =
      training.kind === "multipleChoice" ? shownText(answer.choice).trim() : "";
    if (value === undefined || (training.kind === "multipleChoice" && !choice))
      return [];
    return [
      {
        value,
        points: Number.isFinite(answer.points) ? Math.max(0, answer.points) : 0,
        feedback: shownText(answer.feedback),
        label:
          training.kind === "moveInput"
            ? (entry.trainingMoves?.[index] ?? String(value))
            : choice,
      },
    ];
  });
  if (
    !answers.length ||
    answers.length !== training.answers.length ||
    !["moveInput", "multipleChoice"].includes(training.kind)
  )
    return null;
  const maxPoints = Math.max(...answers.map((a) => a.points));
  if (maxPoints <= 0) return null;
  return {
    key: JSON.stringify([game.tags, game.startFen, path, training]),
    label,
    node,
    fen: position.fen,
    turn: position.turn,
    legalUci: position.legalUci,
    training,
    answers,
    maxPoints,
  };
}

/** Returns null for illegal input. A legal move absent from the annotation is ungraded. */
export function gradeAnswer(
  question: ReplayQuestion,
  value: string | number,
): ReplayGrade | null {
  if (
    question.training.kind === "moveInput" &&
    (typeof value !== "string" || !question.legalUci.includes(value))
  )
    return null;
  if (
    question.training.kind === "multipleChoice" &&
    (typeof value !== "number" ||
      value < 0 ||
      value >= question.training.answers.length)
  )
    return null;
  const answer = question.answers.find((item) => item.value === value);
  if (!answer)
    return {
      kind: "ungraded",
      points: 0,
      feedback: "This legal move has no authored grade.",
      maxPoints: question.maxPoints,
    };
  if (answer.points <= 0)
    return {
      kind: "authoredWrong",
      points: 0,
      feedback:
        answer.feedback ||
        shownText(question.training.wrong) ||
        "The author marked this answer wrong.",
      maxPoints: question.maxPoints,
    };
  return {
    kind: "correct",
    points: answer.points,
    feedback: answer.feedback,
    maxPoints: question.maxPoints,
  };
}

export function recordResult(
  progress: ReplayProgressMap,
  question: ReplayQuestion,
  cleanFullCredit: boolean,
  points: number,
): ReplayProgressMap {
  const old = progress[question.key];
  return {
    ...progress,
    [question.key]: {
      attempts: (old?.attempts ?? 0) + 1,
      needsRetry: !cleanFullCredit,
      bestPoints: Math.max(old?.bestPoints ?? 0, points),
    },
  };
}

export function summary(
  questions: Pick<ReplayQuestion, "key">[],
  progress: ReplayProgressMap,
) {
  return questions.reduce(
    (result, question) => {
      const state = progress[question.key];
      if (!state?.attempts) result.unattempted++;
      else if (state.needsRetry) result.needsRetry++;
      else result.solved++;
      return result;
    },
    { solved: 0, needsRetry: 0, unattempted: 0 },
  );
}
