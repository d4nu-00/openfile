import { useEffect, useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type { SrsGrade, SrsQueueResponse, SrsStudyCard } from "../api/types";
import {
  OPPONENT_DELAY_MS,
  arrowOf,
  clock,
  effectiveGrade,
  finished,
  giveHint,
  intervalLabel,
  isTraineePly,
  opponentPlays,
  startRun,
  tryMove,
  type SrsRun,
} from "../state/srs";
import { Board } from "./Board";

const grades: { grade: SrsGrade; label: string; key: string }[] = [
  { grade: "again", label: "Again", key: "1" },
  { grade: "hard", label: "Hard", key: "2" },
  { grade: "good", label: "Good", key: "3" },
  { grade: "easy", label: "Easy", key: "4" },
];
const promotionNames = {
  q: "Queen",
  r: "Rook",
  b: "Bishop",
  n: "Knight",
} as const;

const explain = (err: unknown) =>
  (err as { message?: string })?.message ?? String(err);
const clockText = (ms: number) => {
  const seconds = Math.max(0, Math.round(ms / 1000));
  return `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
};

/**
 * Studying a deck: the board shows a line, the opponent's moves are played
 * after a moment and the trainee plays theirs. Mistakes bring the card back
 * soon; the grade is chosen after the line, or fixed by what happened.
 */
export function SrsStudy({
  api,
  deckId,
  deckName,
  onClose,
}: {
  api: WorkbenchApi;
  deckId: string;
  deckName: string;
  onClose: () => void;
}) {
  const [session, setSession] = useState<SrsQueueResponse>();
  const [run, setRun] = useState<SrsRun>(startRun());
  const [selected, setSelected] = useState<string>();
  const [promotions, setPromotions] = useState<string[]>([]);
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [studied, setStudied] = useState(0);
  const [again, setAgain] = useState(0);
  const [ended, setEnded] = useState<{ done: boolean; millis: number }>();
  const started = useRef(Date.now());
  const shown = useRef(Date.now());
  const study: SrsStudyCard | undefined = session?.next ?? undefined;

  /** A new queue: the first card is played from its start. */
  const apply = (response: SrsQueueResponse) => {
    setSession(response);
    setRun(startRun());
    setSelected(undefined);
    setPromotions([]);
    shown.current = Date.now();
    if (!response.next)
      setEnded({ done: true, millis: Date.now() - started.current });
  };

  useEffect(() => {
    let live = true;
    const { now, utcOffset } = clock();
    api
      .srsQueue(deckId, now, utcOffset)
      .then((response) => live && apply(response))
      .catch((e) => live && setError(explain(e)));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [api, deckId]);

  const complete = !!study && finished(study, run);
  const traineeTurn = !!study && !complete && isTraineePly(study, run.ply);

  // The opponent's moves are played by themselves, a moment apart.
  useEffect(() => {
    if (!study || ended || complete || traineeTurn) return;
    const timer = window.setTimeout(
      () => setRun((r) => opponentPlays(r)),
      OPPONENT_DELAY_MS,
    );
    return () => window.clearTimeout(timer);
  }, [study, run.ply, ended, complete, traineeTurn]);

  const play = (uci: string) => {
    if (!study || !traineeTurn || busy) return;
    setSelected(undefined);
    setPromotions([]);
    setRun((r) => tryMove(study, r, uci));
  };
  const legal = traineeTurn ? study.plies[run.ply].legalUci : [];
  const clickSquare = (to: string, from = selected) => {
    if (!traineeTurn) return;
    const matches = legal.filter(
      (uci) => uci.slice(0, 2) === from && uci.slice(2, 4) === to,
    );
    if (matches.length > 1) setPromotions(matches);
    else if (matches.length === 1) play(matches[0]);
    else
      setSelected(legal.some((uci) => uci.slice(0, 2) === to) ? to : undefined);
  };

  const submit = async (chosen: SrsGrade) => {
    if (!study || busy) return;
    const grade = effectiveGrade(chosen, run.mistakes, run.hints);
    setBusy(true);
    setError(undefined);
    try {
      const { now, utcOffset } = clock();
      const response = await api.srsAnswer(
        deckId,
        study.card.id,
        grade,
        run.mistakes,
        run.hints,
        Date.now() - shown.current,
        now,
        utcOffset,
      );
      setStudied((n) => n + 1);
      if (grade === "again") setAgain((n) => n + 1);
      apply(response);
    } catch (e) {
      setError(explain(e));
    } finally {
      setBusy(false);
    }
  };
  const suspend = async () => {
    if (!study || busy) return;
    setBusy(true);
    setError(undefined);
    try {
      await api.srsSuspend(deckId, study.card.id, true);
      const { now, utcOffset } = clock();
      apply(await api.srsQueue(deckId, now, utcOffset));
    } catch (e) {
      setError(explain(e));
    } finally {
      setBusy(false);
    }
  };
  const end = () => {
    setEnded((e) => e ?? { done: false, millis: Date.now() - started.current });
  };

  // What the line's result allows: a new card is learnt (Good), a mistake is
  // Again, a hint is Hard at most.
  const lesson = !!study?.learn;
  const failed = run.mistakes > 0;
  const allowed = (grade: SrsGrade) =>
    lesson
      ? grade === "good"
      : failed
        ? grade === "again"
        : !(run.hints > 0 && (grade === "good" || grade === "easy"));
  const defaultGrade: SrsGrade = lesson
    ? "good"
    : failed
      ? "again"
      : run.hints > 0
        ? "hard"
        : "good";

  const latest = useRef({ submit, allowed, defaultGrade, end, ended });
  latest.current = { submit, allowed, defaultGrade, end, ended };
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const now = latest.current;
      if ((event.target as HTMLElement)?.closest?.("input,textarea,select"))
        return;
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (event.key === "Escape") {
        event.preventDefault();
        if (now.ended) onClose();
        else now.end();
        return;
      }
      if (!complete || now.ended) return;
      const grade = grades.find((g) => g.key === event.key)?.grade;
      if (grade) {
        event.preventDefault();
        if (now.allowed(grade)) void now.submit(grade);
      } else if (event.key === "Enter") {
        event.preventDefault();
        event.stopPropagation();
        void now.submit(now.defaultGrade);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [complete, onClose]);

  const arrow = study ? arrowOf(study, run) : undefined;
  const fen = study
    ? run.ply < study.plies.length
      ? study.plies[run.ply].fen
      : study.endFen
    : "";
  const queue = session?.queue;
  const preview = (grade: SrsGrade) =>
    study?.previews.find((p) => p.grade === grade);

  return (
    <div className="replay-overlay srs-overlay" role="presentation">
      <section
        className="replay-session srs-study"
        role="dialog"
        aria-modal="true"
        aria-label={`Study ${deckName}`}
      >
        <header className="replay-header">
          <div>
            <h2>{deckName}</h2>
            <span>Spaced repetition</span>
          </div>
          {queue && !ended && (
            <div className="srs-counts" role="status" aria-label="Cards left">
              <span>New {queue.newDue}</span>
              <span>Learning {queue.learningDue}</span>
              <span>Due {queue.reviewDue}</span>
            </div>
          )}
          <button type="button" onClick={ended ? onClose : end}>
            {ended ? "Close" : "End session"}
          </button>
        </header>
        {error && (
          <p className="replay-error" role="alert">
            {error}
          </p>
        )}
        {ended ? (
          <div className="replay-summary srs-summary">
            <h3>{ended.done ? "Nothing more is due" : "Session ended"}</h3>
            <p>Cards studied: {studied}</p>
            <p>Again: {again}</p>
            <p>Time: {clockText(ended.millis)}</p>
            {ended.done && queue?.nextLearningDue != null && (
              <p>
                The next card is due in{" "}
                {intervalLabel(Math.max(0, queue.nextLearningDue - Date.now()))}
                .
              </p>
            )}
          </div>
        ) : !study ? (
          !error && (
            <div className="replay-loading" role="status">
              Loading the cards…
            </div>
          )
        ) : (
          <>
            <div className="replay-meta">
              <span>{study.card.sourceLabel}</span>
              <span>
                {study.card.white} – {study.card.black}
              </span>
              <span>
                {[study.card.event, study.card.date]
                  .filter((s) => s && s !== "?" && s !== "????.??.??")
                  .join(", ")}
              </span>
              <span>
                {study.card.trainee === "white" ? "White" : "Black"} to play
              </span>
              {lesson && <span className="srs-badge">Learn</span>}
              {queue?.ahead && <span>Learning ahead</span>}
            </div>
            <div className="replay-body">
              <div className="replay-board">
                <Board
                  fen={fen}
                  flipped={study.card.trainee === "black"}
                  legalUci={legal}
                  selectedSquare={selected}
                  onSquare={clickSquare}
                  onMove={
                    traineeTurn
                      ? (from, to) => clickSquare(to, from)
                      : undefined
                  }
                  compact
                  marks={
                    arrow
                      ? {
                          arrows: [
                            {
                              ...arrow,
                              color: run.note === "mistake" ? "R" : "G",
                            },
                          ],
                        }
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
                            play(promotions.find((uci) => uci.endsWith(role))!)
                          }
                        >
                          {promotionNames[role]}
                        </button>
                      ))}
                    <button type="button" onClick={() => setPromotions([])}>
                      Cancel
                    </button>
                  </div>
                )}
              </div>
              <div className="replay-content">
                {complete ? (
                  <h3>Line complete</h3>
                ) : traineeTurn ? (
                  <h3>
                    {study.card.trainee === "white" ? "White" : "Black"} to move
                  </h3>
                ) : (
                  <h3>Opponent's move…</h3>
                )}
                {run.note && (
                  <p
                    className={`replay-feedback srs-${run.note}`}
                    role="status"
                  >
                    {run.note === "mistake"
                      ? "Not this line's move"
                      : run.note === "other"
                        ? "That move belongs to another line in this deck — play this line's move"
                        : "Play the highlighted move"}
                  </p>
                )}
                {study.card.moves.slice(0, run.ply).some((m) => m.comment) && (
                  <ul className="srs-comments" aria-label="Comments">
                    {study.card.moves.slice(0, run.ply).flatMap((m, i) =>
                      m.comment
                        ? [
                            <li key={i}>
                              <strong>{m.san}</strong> <span>{m.comment}</span>
                            </li>,
                          ]
                        : [],
                    )}
                  </ul>
                )}
                {complete && (
                  <div className="srs-grades">
                    {lesson || failed ? (
                      <>
                        <p>
                          {lesson
                            ? `Learned. This card comes back in ${intervalLabel(preview("good")?.dueInMs ?? 0)}.`
                            : "This card is marked Again because of a mistake."}
                        </p>
                        <button
                          type="button"
                          disabled={busy}
                          onClick={() => void submit(defaultGrade)}
                        >
                          Next card
                        </button>
                      </>
                    ) : (
                      <div className="srs-grade-buttons" role="group">
                        {grades.map(({ grade, label, key }) => (
                          <button
                            type="button"
                            key={grade}
                            disabled={busy || !allowed(grade)}
                            className={grade === defaultGrade ? "default" : ""}
                            aria-keyshortcuts={key}
                            onClick={() => void submit(grade)}
                          >
                            <span>{label}</span>
                            <small>
                              {intervalLabel(preview(grade)?.dueInMs ?? 0)}
                            </small>
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                )}
                <div className="replay-actions">
                  <button
                    type="button"
                    disabled={!traineeTurn || lesson || busy}
                    onClick={() => setRun((r) => giveHint(study, r))}
                  >
                    Hint
                  </button>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => void suspend()}
                  >
                    Suspend card
                  </button>
                </div>
              </div>
            </div>
          </>
        )}
      </section>
    </div>
  );
}
