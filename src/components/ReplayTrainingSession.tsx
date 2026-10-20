import { useEffect, useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type { EntrySummary, GameView } from "../api/types";
import { Board } from "./Board";
import { shownText } from "./TrainingDialog";
import {
  collectQuestionNodes,
  gradeAnswer,
  prepareQuestion,
  recordResult,
  summary,
  type ReplayGrade,
  type ReplayProgressMap,
  type ReplayQuestion,
} from "../state/replayTraining";

export type ReplaySource =
  | { kind: "current"; game: GameView; documentId: string; label: string }
  | {
      kind: "selection";
      databaseId: string;
      ranges: [number, number][];
      entry: (row: number) => Promise<EntrySummary | undefined>;
    };

export function ReplayTrainingSession({
  api,
  source,
  onClose,
}: {
  api: WorkbenchApi;
  source: ReplaySource;
  onClose: () => void;
}) {
  const [questions, setQuestions] = useState<ReplayQuestion[]>([]);
  const [loading, setLoading] = useState(true);
  const [scanned, setScanned] = useState(0);
  const [skipped, setSkipped] = useState(0);
  const [progress, setProgress] = useState<ReplayProgressMap>({});
  const progressRef = useRef<ReplayProgressMap>({});
  const [sessionProgress, setSessionProgress] = useState<ReplayProgressMap>({});
  const sessionProgressRef = useRef<ReplayProgressMap>({});
  const runRecorded = useRef(false);
  const pendingWrite = useRef<Promise<void>>(Promise.resolve());
  const [error, setError] = useState<string>();
  const [storageError, setStorageError] = useState<string>();
  const [index, setIndex] = useState(0);
  const [epoch, setEpoch] = useState(0);
  const [selected, setSelected] = useState<string>();
  const [promotions, setPromotions] = useState<string[]>([]);
  const [helpLevel, setHelpLevel] = useState(0);
  const [elapsed, setElapsed] = useState(0);
  const [assisted, setAssisted] = useState(false);
  const [grade, setGrade] = useState<ReplayGrade>();
  const [done, setDone] = useState(false);
  const [outcome, setOutcome] = useState<string>();
  const cancel = useRef(false);
  const question = questions[index];
  const timedOut =
    !!question?.training.seconds && elapsed >= question.training.seconds;

  useEffect(() => {
    let live = true;
    cancel.current = false;
    const addGame = async (
      game: GameView,
      documentId: string,
      label: string,
      sink: ReplayQuestion[],
    ) => {
      const nodes = collectQuestionNodes(game);
      for (const node of nodes) {
        if (!live || cancel.current) return;
        const parent = game.nodes[node].parent;
        if (parent === undefined) {
          setSkipped((n) => n + 1);
          continue;
        }
        try {
          const position = await api.positionAt(
            documentId,
            parent,
            `replay-${documentId}-${node}`,
          );
          if (!live || cancel.current) return;
          const item = prepareQuestion(
            game,
            node,
            position,
            `${label} · question ${nodes.indexOf(node) + 1}`,
          );
          if (item) sink.push(item);
          else setSkipped((n) => n + 1);
        } catch {
          if (live) setSkipped((n) => n + 1);
        }
      }
    };
    const run = async () => {
      try {
        try {
          const saved = await api.replayTrainingProgress();
          if (live) {
            progressRef.current = saved;
            setProgress(saved);
          }
        } catch {
          if (live)
            setStorageError(
              "Progress could not be loaded; results may not persist.",
            );
        }
        const found: ReplayQuestion[] = [];
        if (source.kind === "current") {
          await addGame(source.game, source.documentId, source.label, found);
          if (live) setScanned(1);
        } else {
          for (const [from, to] of source.ranges) {
            for (let row = from; row < to; row++) {
              if (!live || cancel.current) return;
              let loaded:
                Awaited<ReturnType<WorkbenchApi["loadEntry"]>> | undefined;
              try {
                const entry = await source.entry(row);
                if (!live || cancel.current) return;
                if (entry?.kind === "game" && !entry.deleted) {
                  loaded = await api.loadEntry(
                    source.databaseId,
                    entry.ordinal,
                    `replay-row-${row}`,
                  );
                  if (loaded.game && loaded.documentId)
                    await addGame(
                      loaded.game,
                      loaded.documentId,
                      `${entry.white ?? "White"}–${entry.black ?? "Black"} (#${entry.ordinal})`,
                      found,
                    );
                  else if (live) setSkipped((n) => n + 1);
                }
              } catch {
                if (live) setSkipped((n) => n + 1);
              } finally {
                if (loaded?.documentId)
                  await api.closeDocument(loaded.documentId).catch(() => {});
              }
              if (live) setScanned((n) => n + 1);
            }
          }
        }
        if (live && !cancel.current) setQuestions(found);
      } catch (e) {
        if (live) setError((e as { message?: string })?.message ?? String(e));
      } finally {
        if (live) setLoading(false);
      }
    };
    void run();
    return () => {
      live = false;
      cancel.current = true;
    };
  }, [api, source]);

  useEffect(() => {
    runRecorded.current = false;
    setSelected(undefined);
    setPromotions([]);
    setHelpLevel(0);
    setElapsed(0);
    setAssisted(false);
    setGrade(undefined);
    setDone(false);
    setOutcome(undefined);
  }, [index, epoch]);

  useEffect(() => {
    if (!question || done) return;
    const timer = window.setInterval(
      () => setElapsed((value) => value + 1),
      1000,
    );
    return () => window.clearInterval(timer);
  }, [question, done, epoch]);

  const save = (clean: boolean, points: number) => {
    if (!question) return;
    const updated = (current: ReplayProgressMap) =>
      runRecorded.current
        ? {
            ...current,
            [question.key]: {
              ...current[question.key],
              needsRetry: !clean,
              bestPoints: Math.max(
                current[question.key]?.bestPoints ?? 0,
                points,
              ),
            },
          }
        : recordResult(current, question, clean, points);
    const next = updated(progressRef.current);
    const nextSession = updated(sessionProgressRef.current);
    runRecorded.current = true;
    progressRef.current = next;
    sessionProgressRef.current = nextSession;
    setProgress(next);
    setSessionProgress(nextSession);
    pendingWrite.current = pendingWrite.current
      .then(() => api.replayTrainingRecord(question.key, next[question.key]))
      .catch(() => {
        setStorageError("Progress could not be saved to the app data folder.");
      });
  };
  useEffect(() => {
    if (!question || done || question.training.seconds <= 0) return;
    const timeout = window.setTimeout(() => {
      setElapsed((value) => Math.max(value, question.training.seconds));
      if (!runRecorded.current) save(false, 0);
    }, question.training.seconds * 1000);
    return () => window.clearTimeout(timeout);
  }, [question, done, epoch]);
  const answer = (value: string | number) => {
    if (!question || done) return;
    const result = gradeAnswer(question, value);
    if (!result) return;
    setSelected(undefined);
    setPromotions([]);
    setGrade(result);
    const clean =
      result.kind === "correct" &&
      result.points >= result.maxPoints &&
      !assisted &&
      !timedOut;
    save(clean, result.points);
    if (result.kind === "correct") {
      setDone(true);
      setOutcome(clean ? "Solved" : "Needs another attempt");
    } else {
      setAssisted(true);
      setOutcome(
        result.kind === "authoredWrong"
          ? "Authored wrong answer"
          : "Ungraded legal move",
      );
    }
  };
  const clickSquare = (to: string) => {
    if (!question || done || question.training.kind !== "moveInput") return;
    const matches = question.legalUci.filter(
      (uci) => uci.slice(0, 2) === selected && uci.slice(2, 4) === to,
    );
    if (matches.length > 1) setPromotions(matches);
    else if (matches.length === 1) answer(matches[0]);
    else
      setSelected(
        question.legalUci.some((uci) => uci.slice(0, 2) === to)
          ? to
          : undefined,
      );
  };
  const reset = () => setEpoch((n) => n + 1);
  const total =
    source.kind === "selection"
      ? source.ranges.reduce((n, [a, b]) => n + b - a, 0)
      : 1;
  const totals = summary(questions, sessionProgress);

  return (
    <div className="replay-overlay" role="presentation">
      <section
        className="replay-session"
        role="dialog"
        aria-modal="true"
        aria-label="Replay Training Session"
      >
        <header className="replay-header">
          <div>
            <h2>Replay Training</h2>
            <span>Practise saved questions</span>
          </div>
          <button
            type="button"
            onClick={() => {
              cancel.current = true;
              onClose();
            }}
          >
            Close
          </button>
        </header>
        {storageError && (
          <p className="replay-error" role="alert">
            {storageError}
          </p>
        )}
        {error && (
          <p className="replay-error" role="alert">
            {error}
          </p>
        )}
        {loading ? (
          <div className="replay-loading" role="status">
            <p>
              Scanning games… {scanned} of {total}
            </p>
            <p>{skipped} questions or games skipped</p>
            <button
              type="button"
              onClick={() => {
                cancel.current = true;
                onClose();
              }}
            >
              Cancel scan
            </button>
          </div>
        ) : questions.length === 0 ? (
          <div className="replay-empty">
            <h3>No usable training questions</h3>
            <p>
              {skipped
                ? `${skipped} stale or malformed questions or games were skipped.`
                : "This game or selection has no saved training questions."}
            </p>
          </div>
        ) : !question ? (
          <div className="replay-summary">
            <h3>Session summary</h3>
            <p>
              {totals.solved} solved · {totals.needsRetry} need another attempt
              · {totals.unattempted} not attempted
            </p>
            {totals.needsRetry > 0 && (
              <>
                <h4>Try again</h4>
                <ul>
                  {questions
                    .filter((q) => sessionProgress[q.key]?.needsRetry)
                    .map((q, i) => (
                      <li key={`${q.key}-${i}`}>{q.label}</li>
                    ))}
                </ul>
              </>
            )}
            <button type="button" onClick={() => setIndex(0)}>
              Practise again
            </button>
          </div>
        ) : (
          <>
            <div className="replay-meta">
              <span>
                {index + 1} of {questions.length}
              </span>
              <span>{question.label}</span>
              <span>
                {question.turn === "white" ? "White" : "Black"} to move
              </span>
            </div>
            <div className="replay-body">
              <div className="replay-board">
                <Board
                  fen={question.fen}
                  flipped={question.turn === "black"}
                  legalUci={question.legalUci}
                  selectedSquare={selected}
                  onSquare={clickSquare}
                  compact
                  marks={
                    helpLevel > 0
                      ? question.training.help?.[helpLevel - 1]
                      : undefined
                  }
                />
                {promotions.length > 0 && (
                  <div
                    className="practice-promotion"
                    aria-label="Choose promotion"
                  >
                    {(["q", "r", "b", "n"] as const)
                      .filter((role) =>
                        promotions.some((uci) => uci.endsWith(role)),
                      )
                      .map((role) => (
                        <button
                          type="button"
                          key={role}
                          onClick={() =>
                            answer(
                              promotions.find((uci) => uci.endsWith(role))!,
                            )
                          }
                        >
                          {
                            { q: "Queen", r: "Rook", b: "Bishop", n: "Knight" }[
                              role
                            ]
                          }
                        </button>
                      ))}
                    <button type="button" onClick={() => setPromotions([])}>
                      Cancel
                    </button>
                  </div>
                )}
              </div>
              <div className="replay-content">
                <h3>{shownText(question.training.question)}</h3>
                <div className="replay-facts">
                  {question.training.score > 0 && (
                    <span>{question.training.score} question points</span>
                  )}
                  {question.maxPoints > 0 && (
                    <span>{question.maxPoints} top answer points</span>
                  )}
                  {question.training.seconds > 0 && (
                    <span>
                      {timedOut
                        ? "Time expired"
                        : `${Math.max(0, question.training.seconds - elapsed)}s remaining`}
                    </span>
                  )}
                  <span>{elapsed}s elapsed</span>
                </div>
                {question.training.kind === "multipleChoice" ? (
                  <div className="replay-choices">
                    {question.answers.map((choice) => (
                      <button
                        type="button"
                        key={choice.value}
                        disabled={done}
                        onClick={() => answer(choice.value)}
                      >
                        {choice.label}
                      </button>
                    ))}
                  </div>
                ) : (
                  <p className="hint">Play a legal move on the board.</p>
                )}
                {helpLevel > 0 && !done && (
                  <p className="replay-hint">
                    Hint {helpLevel}:{" "}
                    {shownText(question.training.help?.[helpLevel - 1]) ||
                      "See the board marks."}
                  </p>
                )}
                {grade && (
                  <div className="replay-feedback" role="status">
                    <strong>{outcome}</strong>
                    <p>
                      {grade.kind === "correct"
                        ? `${grade.points} of ${grade.maxPoints} points.`
                        : grade.kind === "authoredWrong"
                          ? "The author graded this answer as wrong."
                          : "This legal move has no authored grade; no engine assessment was made."}
                    </p>
                    {grade.feedback && <p>{grade.feedback}</p>}
                  </div>
                )}
                {!grade && outcome && (
                  <div className="replay-feedback" role="status">
                    <strong>{outcome}</strong>
                    {outcome === "Revealed" && (
                      <ul>
                        {question.answers
                          .filter((a) => a.points > 0)
                          .map((a, i) => (
                            <li key={i}>
                              {question.training.kind === "multipleChoice"
                                ? a.label
                                : a.value}{" "}
                              — {a.points} points
                              {a.feedback ? ` · ${a.feedback}` : ""}
                            </li>
                          ))}
                      </ul>
                    )}
                  </div>
                )}
                <div className="replay-actions">
                  <button
                    type="button"
                    disabled={
                      done ||
                      helpLevel >= 2 ||
                      (!shownText(question.training.help?.[helpLevel]) &&
                        !question.training.help?.[helpLevel]?.arrows?.length &&
                        !question.training.help?.[helpLevel]?.squares?.length)
                    }
                    onClick={() => {
                      setHelpLevel(helpLevel + 1);
                      setAssisted(true);
                      save(false, 0);
                    }}
                  >
                    Hint {helpLevel < 2 ? helpLevel + 1 : ""}
                  </button>
                  <button
                    type="button"
                    disabled={done}
                    onClick={() => {
                      setDone(true);
                      setOutcome("Revealed");
                      setGrade(undefined);
                      save(false, 0);
                    }}
                  >
                    Reveal
                  </button>
                  <button
                    type="button"
                    disabled={done}
                    onClick={() => {
                      setDone(true);
                      setOutcome("Skipped — needs another attempt");
                      setGrade(undefined);
                      save(false, 0);
                    }}
                  >
                    Skip
                  </button>
                  {(grade || done || helpLevel > 0) && (
                    <button type="button" onClick={reset}>
                      Try again
                    </button>
                  )}
                </div>
              </div>
            </div>
            <footer className="replay-footer">
              <span>
                {sessionProgress[question.key]?.needsRetry
                  ? "Needs another attempt"
                  : sessionProgress[question.key]?.attempts
                    ? "Solved"
                    : progress[question.key]?.needsRetry
                      ? "Needs another attempt from an earlier session"
                      : progress[question.key]?.attempts
                        ? "Previously solved"
                        : "Not attempted"}
              </span>
              <button type="button" onClick={() => setIndex(index + 1)}>
                Next question
              </button>
            </footer>
          </>
        )}
      </section>
    </div>
  );
}
