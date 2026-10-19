import { useEffect, useMemo, useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type {
  Line,
  WorkspaceAction,
  WorkspaceSide,
  RepertoireGrade,
  RepertoireSnapshot,
} from "../api/types";
import type { BookState } from "../state/positionPanes";

const explain = (e: unknown) =>
  (e as { message?: string })?.message ?? String(e);

export function RepertoirePane({
  api,
  line,
  side,
  onSideChange,
  book,
  onNavigate,
  onPractice,
  onStopPractice,
  practice,
  practising = false,
  refresh = 0,
}: {
  api: WorkbenchApi;
  line?: Line;
  side: WorkspaceSide;
  onSideChange?: (side: WorkspaceSide) => void;
  book: BookState;
  onNavigate: (fen: string) => void;
  onPractice: () => void;
  onStopPractice?: () => void;
  practice?: RepertoireGrade;
  practising?: boolean;
  refresh?: number;
}) {
  const [snapshot, setSnapshot] = useState<RepertoireSnapshot>();
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>();
  const [selected, setSelected] = useState("");
  const key = line ? `${line.startFen}|${line.moves.join(" ")}` : "";
  const context = `${key}|${side}`;
  const contextRef = useRef(context);
  contextRef.current = context;
  useEffect(() => {
    setSaving(false);
    if (!line) {
      setSnapshot(undefined);
      return;
    }
    let live = true;
    setSnapshot(undefined);
    setLoading(true);
    setError(undefined);
    void api
      .repertoireSnapshot(line, side)
      .then((value) => {
        if (live) setSnapshot(value);
      })
      .catch((e) => {
        if (live) setError(explain(e));
      })
      .finally(() => {
        if (live) setLoading(false);
      });
    return () => {
      live = false;
    };
  }, [api, key, side, refresh]);

  async function edit(uci: string, action: WorkspaceAction, note?: string) {
    if (!line || saving) return;
    const startedAt = context;
    setSaving(true);
    setError(undefined);
    try {
      const updated = await api.repertoireEdit(line, side, uci, action, note);
      if (contextRef.current === startedAt) setSnapshot(updated);
    } catch (e) {
      if (contextRef.current === startedAt) setError(explain(e));
    } finally {
      if (contextRef.current === startedAt) setSaving(false);
    }
  }

  const saved = snapshot?.current?.moves ?? [];
  const savedUci = useMemo(() => new Set(saved.map((m) => m.uci)), [saved]);
  const reference = book.loading || !snapshot ? undefined : book.tree;
  const uncovered =
    reference?.moves.filter((m) =>
      snapshot?.ownTurn
        ? !savedUci.has(m.uci)
        : !snapshot?.answeredReplies?.[m.uci],
    ) ?? [];
  const available = snapshot?.legal.filter((m) => !savedUci.has(m.uci)) ?? [];
  const selectedMove =
    available.find((m) => m.uci === selected) ?? available[0];
  const referenceLabel =
    book.loading || !snapshot
      ? "Reference data loading; coverage is pending."
      : !reference ||
          ["noTree", "notInTree", "tooMany"].includes(reference.status)
        ? "Reference data missing for this position. Coverage cannot be measured."
        : reference.status === "line"
          ? "Reference data limited: followed one route from the last stored tree position; transposed games may be missing."
          : reference.status === "filtered"
            ? `Sampled search result: ${reference.sampledHits ?? 0} of ${reference.totalHits ?? 0} hits scanned. Counts describe this sample.`
            : "Reference counts from this database’s opening tree.";

  return (
    <div className="repertoire-pane" aria-label="Repertoire workspace">
      <div className="pane-header repertoire-header">
        <strong>Repertoire</strong>
        <span
          className="book-scope"
          role="group"
          aria-label="Repertoire colour"
        >
          {(["white", "black"] as const).map((colour) => (
            <button
              key={colour}
              type="button"
              className={side === colour ? "active" : ""}
              aria-pressed={side === colour}
              onClick={() => onSideChange?.(colour)}
            >
              {colour === "white" ? "White" : "Black"}
            </button>
          ))}
        </span>
        {loading && <span className="hint">Loading…</span>}
      </div>
      {!line ? (
        <div className="empty-state">
          Open a game or new board to choose a position.
        </div>
      ) : (
        <>
          {error && (
            <p role="alert" className="repertoire-error">
              {error}
            </p>
          )}
          <div className="repertoire-scroll">
            <section className="repertoire-section">
              <h3>
                {snapshot?.ownTurn
                  ? "My choices"
                  : "Opponent replies in my lines"}
              </h3>
              {practising || practice ? (
                <>
                  <p className="hint">
                    Play a legal move on the board. Saved choices are hidden
                    during practice.
                  </p>
                  {practice && (
                    <p
                      role="status"
                      className={
                        practice.correct
                          ? "repertoire-correct"
                          : "repertoire-miss"
                      }
                    >
                      {practice.correct
                        ? `Correct: ${practice.playedSan}.`
                        : `${practice.playedSan} is not saved. Preferred: ${practice.preferredSan ?? "—"}. Needs another attempt.`}
                    </p>
                  )}
                  {practice && !practice.correct && (
                    <button onClick={onPractice}>Try again</button>
                  )}
                  {onStopPractice && (
                    <button onClick={onStopPractice}>End practice</button>
                  )}
                </>
              ) : (
                <>
                  {saved.length === 0 && (
                    <p className="hint">No moves saved at this position.</p>
                  )}
                  <div className="repertoire-choices">
                    {saved.map((m) => (
                      <div className="repertoire-choice" key={m.uci}>
                        <span className="book-move">{m.san}</span>
                        {m.preferred && (
                          <span className="badge">Preferred</span>
                        )}
                        {!m.valid && (
                          <span
                            className="badge"
                            title="This saved move is no longer legal here"
                          >
                            Invalid saved move
                          </span>
                        )}
                        {!m.preferred && m.valid && (
                          <button
                            disabled={saving}
                            onClick={() => void edit(m.uci, "prefer")}
                          >
                            Make preferred
                          </button>
                        )}
                        <button
                          disabled={saving}
                          aria-label={`Remove ${m.san}`}
                          onClick={() => void edit(m.uci, "remove")}
                        >
                          Remove
                        </button>
                        <input
                          aria-label={`Note for ${m.san}`}
                          key={`${m.uci}:${m.note}`}
                          defaultValue={m.note}
                          maxLength={280}
                          placeholder="Short note"
                          disabled={saving || !m.valid}
                          onBlur={(ev) => {
                            if (ev.target.value !== m.note)
                              void edit(m.uci, "note", ev.target.value);
                          }}
                        />
                      </div>
                    ))}
                  </div>
                  {available.length > 0 && (
                    <div className="repertoire-add">
                      <label htmlFor="repertoire-move">Add a legal move</label>
                      <select
                        id="repertoire-move"
                        value={selectedMove.uci}
                        onChange={(ev) => setSelected(ev.target.value)}
                      >
                        {available.map((m) => (
                          <option key={m.uci} value={m.uci}>
                            {m.san}
                          </option>
                        ))}
                      </select>
                      <button
                        disabled={saving}
                        aria-label={`Save ${selectedMove.san}`}
                        onClick={() => void edit(selectedMove.uci, "add")}
                      >
                        Save move
                      </button>
                    </div>
                  )}
                  {snapshot?.ownTurn && saved.some((m) => m.valid) && (
                    <button onClick={onPractice}>Practise this position</button>
                  )}
                </>
              )}
            </section>
            <section className="repertoire-section">
              <h3>
                {snapshot?.ownTurn
                  ? "Other reference moves"
                  : "Uncovered opponent replies"}
              </h3>
              <p className="hint">{referenceLabel}</p>
              {book.loading && (
                <p className="hint">Updating reference counts…</p>
              )}
              {uncovered.length > 0 ? (
                <div className="repertoire-coverage">
                  {uncovered.map((m) => (
                    <div key={m.uci}>
                      <span>
                        {m.san} · {m.games} games
                      </span>
                      {!practising && !practice && snapshot?.ownTurn && (
                        <button onClick={() => void edit(m.uci, "add")}>
                          Add {m.san} from reference
                        </button>
                      )}
                      {!practising &&
                        !practice &&
                        !snapshot?.ownTurn &&
                        snapshot?.replyPositions?.[m.uci] && (
                          <button
                            onClick={() =>
                              onNavigate(snapshot.replyPositions![m.uci])
                            }
                          >
                            Answer {m.san}
                          </button>
                        )}
                    </div>
                  ))}
                </div>
              ) : (
                reference?.status === "tree" && (
                  <p className="hint">
                    No uncovered reference moves at this position.
                  </p>
                )
              )}
            </section>
            <section className="repertoire-section">
              <h3>Saved positions · {snapshot?.entries.length ?? 0}</h3>
              {(snapshot?.entries.length ?? 0) === 0 ? (
                <p className="hint">Save a move to begin a line.</p>
              ) : (
                <div className="repertoire-lines">
                  {snapshot?.entries.map((entry) => (
                    <button
                      key={entry.key}
                      className={
                        entry.key === snapshot.current?.key ? "active" : ""
                      }
                      onClick={() => onNavigate(entry.fen)}
                      title="Open this position on a new board"
                    >
                      <span>{entry.route}</span>
                      <small>
                        {entry.moves
                          .filter((m) => m.valid)
                          .map((m) => m.san)
                          .join(", ")}
                        {entry.practice.needsRetry ? " · Needs retry" : ""}
                      </small>
                    </button>
                  ))}
                </div>
              )}
            </section>
          </div>
        </>
      )}
    </div>
  );
}
