import { useState } from "react";
import type {
  GameNode,
  Training,
  TrainingAnswer,
  TrainingBlock,
} from "../api/types";

/** The reference app's language tabs, in its slot order. */
export const trainingLanguages = [
  ["en", "English"],
  ["de", "German"],
  ["fr", "French"],
  ["es", "Spanish"],
  ["it", "Italian"],
  ["nl", "Dutch"],
  ["pt", "Portuguese"],
] as const;

const empty = (): TrainingBlock => ({ texts: [] });

/** The block's text in `language` (string index 0). */
export function blockText(block: TrainingBlock | undefined, language: number) {
  return (
    block?.texts.find((t) => t.language === language && !t.index)?.text ?? ""
  );
}

/** The text in `language`, else the first language that has one. */
export function shownText(block: TrainingBlock | undefined, language = 0) {
  return (
    blockText(block, language) ||
    trainingLanguages.map((_, i) => blockText(block, i)).find(Boolean) ||
    ""
  );
}

function withText(
  block: TrainingBlock | undefined,
  language: number,
  text: string,
): TrainingBlock {
  const b = block ?? empty();
  const others = b.texts.filter((t) => t.language !== language || t.index);
  return {
    ...b,
    texts: text ? [...others, { language, index: 0, text }] : others,
  };
}

const marks = (b?: TrainingBlock) =>
  (b?.squares?.length ?? 0) + (b?.arrows?.length ?? 0);

/**
 * Training Annotation (the reference app's `ANNOTRAINING`, Ctrl+Alt+M): a
 * question about the position before the move, with hints and graded
 * answers, edited one language at a time. Texts in other languages and the
 * board marks of hints and answers are kept.
 */
export function TrainingDialog({
  node,
  moveText,
  onSave,
  onClose,
  error,
}: {
  node: GameNode;
  /** "2.Nf3", the move the question is attached to. */
  moveText: string;
  /** Saves (null removes); `moves` are the answers' moves, as typed. */
  onSave: (training: Training | null, moves: (string | null)[]) => void;
  onClose: () => void;
  error?: string;
}) {
  const existing = node.training;
  const [t, setT] = useState<Training>(
    () =>
      structuredClone(existing) ?? {
        kind: "moveInput",
        seconds: 300,
        score: 10,
        question: empty(),
        wrong: empty(),
        help: [empty(), empty()],
        answers: [{ points: 10, feedback: empty() }],
      },
  );
  const [moves, setMoves] = useState<string[]>(() =>
    (existing?.answers ?? [{ points: 10 }]).map(
      (_, i) => node.trainingMoves?.[i] ?? (existing ? "" : (node.san ?? "")),
    ),
  );
  const [language, setLanguage] = useState(0);
  const moveInput = t.kind === "moveInput";
  const setAnswer = (i: number, a: TrainingAnswer) =>
    setT({ ...t, answers: t.answers.map((x, j) => (j === i ? a : x)) });
  const help = t.help ?? [empty(), empty()];
  const kept =
    marks(t.question) +
    help.reduce((n, b) => n + marks(b), 0) +
    t.answers.reduce((n, a) => n + marks(a.feedback) + marks(a.choice), 0);
  const valid = shownText(t.question).trim() !== "";
  const area = (
    label: string,
    block: TrainingBlock | undefined,
    set: (b: TrainingBlock) => void,
    rows = 2,
  ) => (
    <label className="training-field">
      <span>{label}</span>
      <textarea
        aria-label={label}
        rows={rows}
        value={blockText(block, language)}
        placeholder={
          blockText(block, language) ? undefined : shownText(block) || undefined
        }
        onChange={(e) => set(withText(block, language, e.target.value))}
      />
    </label>
  );
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Training Annotation"
        className="modal training"
        onSubmit={(e) => {
          e.preventDefault();
          if (valid)
            onSave(
              t,
              moveInput ? moves.map((m) => (m.trim() ? m.trim() : null)) : [],
            );
        }}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Training Annotation: before {moveText}</h2>
        <div className="training-row">
          <fieldset className="segmented" aria-label="Answer by">
            {(
              [
                ["moveInput", "Move input"],
                ["multipleChoice", "Multiple choice"],
              ] as const
            ).map(([kind, label]) => (
              <label key={kind}>
                <input
                  type="radio"
                  name="training-kind"
                  checked={t.kind === kind}
                  onChange={() => setT({ ...t, kind })}
                />
                {label}
              </label>
            ))}
          </fieldset>
          <label>
            Seconds{" "}
            <input
              type="number"
              aria-label="Seconds"
              min={0}
              max={86400}
              value={t.seconds}
              onChange={(e) =>
                setT({ ...t, seconds: Number(e.target.value) || 0 })
              }
            />
          </label>
          <label>
            Score{" "}
            <input
              type="number"
              aria-label="Score"
              min={0}
              max={100}
              value={t.score}
              onChange={(e) =>
                setT({ ...t, score: Number(e.target.value) || 0 })
              }
            />
          </label>
          <label>
            Language{" "}
            <select
              aria-label="Language"
              value={language}
              onChange={(e) => setLanguage(Number(e.target.value))}
            >
              {trainingLanguages.map(([code, name], i) => (
                <option key={code} value={i}>
                  {name}
                  {shownText(t.question) && blockText(t.question, i)
                    ? " ✓"
                    : ""}
                </option>
              ))}
            </select>
          </label>
        </div>
        {area(
          "Question",
          t.question,
          (question) => setT({ ...t, question }),
          3,
        )}
        {area("Wrong answer", t.wrong, (wrong) => setT({ ...t, wrong }))}
        {area("Hint 1", help[0], (h) => setT({ ...t, help: [h, help[1]] }))}
        {area("Hint 2", help[1], (h) => setT({ ...t, help: [help[0], h] }))}
        <table className="stat-table training-answers" aria-label="Answers">
          <thead>
            <tr>
              <th>{moveInput ? "Move" : "Choice"}</th>
              <th className="num">Points</th>
              <th>Feedback</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {t.answers.map((a, i) => (
              <tr key={i}>
                <td>
                  {moveInput ? (
                    <input
                      aria-label={`Move ${i + 1}`}
                      value={moves[i] ?? ""}
                      placeholder="e.g. Nf3"
                      onChange={(e) =>
                        setMoves(
                          moves.map((m, j) => (j === i ? e.target.value : m)),
                        )
                      }
                    />
                  ) : (
                    <input
                      aria-label={`Choice ${i + 1}`}
                      value={blockText(a.choice, language)}
                      placeholder={shownText(a.choice) || undefined}
                      onChange={(e) =>
                        setAnswer(i, {
                          ...a,
                          choice: withText(a.choice, language, e.target.value),
                        })
                      }
                    />
                  )}
                </td>
                <td className="num">
                  <input
                    type="number"
                    aria-label={`Points ${i + 1}`}
                    min={0}
                    max={255}
                    value={a.points}
                    onChange={(e) =>
                      setAnswer(i, {
                        ...a,
                        points: Math.min(255, Number(e.target.value) || 0),
                      })
                    }
                  />
                </td>
                <td>
                  <input
                    aria-label={`Feedback ${i + 1}`}
                    value={blockText(a.feedback, language)}
                    placeholder={shownText(a.feedback) || undefined}
                    onChange={(e) =>
                      setAnswer(i, {
                        ...a,
                        feedback: withText(
                          a.feedback,
                          language,
                          e.target.value,
                        ),
                      })
                    }
                  />
                </td>
                <td>
                  <button
                    type="button"
                    aria-label={`Remove answer ${i + 1}`}
                    onClick={() => {
                      setT({
                        ...t,
                        answers: t.answers.filter((_, j) => j !== i),
                      });
                      setMoves(moves.filter((_, j) => j !== i));
                    }}
                  >
                    ×
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <div className="training-row">
          <button
            type="button"
            disabled={t.answers.length >= 32}
            onClick={() => {
              setT({
                ...t,
                answers: [...t.answers, { points: 0, feedback: empty() }],
              });
              setMoves([...moves, ""]);
            }}
          >
            Add answer
          </button>
          <span className="hint">
            {moveInput
              ? "Moves are typed in the position before the move."
              : "Each choice can have its own points and feedback."}
            {kept ? ` ${kept} board marks of hints and answers are kept.` : ""}
          </span>
        </div>
        {error && <p role="alert">{error}</p>}
        <div className="mask-buttons">
          <button type="submit" disabled={!valid}>
            Save
          </button>
          {existing && (
            <button type="button" onClick={() => onSave(null, [])}>
              Delete
            </button>
          )}
          <button type="button" onClick={onClose}>
            Close
          </button>
        </div>
      </form>
    </div>
  );
}
