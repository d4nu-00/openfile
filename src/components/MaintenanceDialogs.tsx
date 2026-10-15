import { useState } from "react";
import type { DoubleCompare, DoubleOptions } from "../api/types";

/** ChessBase's defaults for Kill Doubles (`0x1407ab920`). */
export const defaultDoubleOptions: DoubleOptions = {
  decision: "keepBetter",
  names: "similar",
  moves: "similar",
  minMoves: 0,
  ignoreResult: true,
  ignoreYear: false,
  keepAnnotators: false,
  deleteAnnotated: false,
};

function Choice<T extends string>({
  legend,
  name,
  value,
  options,
  onChange,
}: {
  legend: string;
  name: string;
  value: T;
  options: [T, string][];
  onChange: (v: T) => void;
}) {
  return (
    <fieldset className="mask-row">
      <legend className="mask-label">{legend}</legend>
      {options.map(([v, label]) => (
        <label key={v} className="mask-check">
          <input
            type="radio"
            name={name}
            checked={value === v}
            onChange={() => onChange(v)}
          />
          {label}
        </label>
      ))}
    </fieldset>
  );
}

/**
 * Find Double Games: ChessBase's Kill Doubles dialog (894) as far as it is
 * ported. One game of each pair of doubles is marked deleted.
 */
export function DoublesDialog({
  running,
  error,
  onStart,
  onCancel,
  onClose,
}: {
  running?: string;
  error?: string;
  onStart: (options: DoubleOptions) => void;
  onCancel: () => void;
  onClose: () => void;
}) {
  const [o, setO] = useState<DoubleOptions>(defaultDoubleOptions);
  const set = <K extends keyof DoubleOptions>(
    key: K,
    value: DoubleOptions[K],
  ) => setO((x) => ({ ...x, [key]: value }));
  const compare: [DoubleCompare, string][] = [
    ["exact", "Exact"],
    ["similar", "Similar"],
    ["ignore", "Ignore"],
  ];
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Find Double Games"
        className="modal maintenance"
        onSubmit={(e) => {
          e.preventDefault();
          if (!running) onStart(o);
        }}
        onKeyDown={(e) => e.key === "Escape" && !running && onClose()}
      >
        <h2>Find Double Games</h2>
        <fieldset className="maintenance-options" disabled={!!running}>
          <Choice
            legend="Decision"
            name="doubles-decision"
            value={o.decision}
            options={[
              ["first", "Always delete first game"],
              ["second", "Always delete second game"],
              ["keepBetter", "Keep better game"],
            ]}
            onChange={(v) => set("decision", v)}
          />
          <Choice
            legend="Names"
            name="doubles-names"
            value={o.names}
            options={compare}
            onChange={(v) => set("names", v)}
          />
          <Choice
            legend="Moves"
            name="doubles-moves"
            value={o.moves === "ignore" ? "similar" : o.moves}
            options={compare.slice(0, 2)}
            onChange={(v) => set("moves", v)}
          />
          <label className="mask-row">
            <span className="mask-label">Game length</span>
            <input
              type="number"
              aria-label="Game length"
              min={0}
              max={500}
              value={o.minMoves}
              onChange={(e) =>
                set(
                  "minMoves",
                  Math.max(0, Math.min(500, Number(e.target.value) || 0)),
                )
              }
            />
            <span className="hint">moves at least</span>
          </label>
          {(
            [
              ["ignoreResult", "Ignore result"],
              ["ignoreYear", "Ignore year"],
              ["keepAnnotators", "Keep different annotators"],
              ["deleteAnnotated", "Delete annotated games"],
            ] as const
          ).map(([key, label]) => (
            <label key={key} className="mask-check">
              <input
                type="checkbox"
                checked={o[key]}
                onChange={(e) => set(key, e.target.checked)}
              />
              {label}
            </label>
          ))}
        </fieldset>
        <p className="hint">
          The annotated game of a pair is kept. Keep better keeps the game with
          more data (ratings, names, date, source, round, ECO). Two annotated
          doubles are both kept unless Delete annotated games is on. Doubles are
          marked deleted; Remove Deleted Games removes them.
        </p>
        {running && (
          <p role="status" className="hint">
            Comparing: {running}
          </p>
        )}
        {error && <p role="alert">{error}</p>}
        <div className="mask-buttons">
          {running ? (
            <button type="button" onClick={onCancel}>
              Stop
            </button>
          ) : (
            <>
              <button type="submit">Find</button>
              <button type="button" onClick={onClose}>
                Close
              </button>
            </>
          )}
        </div>
      </form>
    </div>
  );
}

/** A maintenance job's progress, then its result (Check Integrity, Remove
 * Deleted Games, and the doubles found). */
export function JobDialog({
  title,
  running,
  error,
  lines,
  onCancel,
  onClose,
}: {
  title: string;
  running?: string;
  error?: string;
  /** The result, one line each. */
  lines?: string[];
  onCancel: () => void;
  onClose: () => void;
}) {
  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className="modal maintenance"
        onKeyDown={(e) => e.key === "Escape" && !running && onClose()}
      >
        <h2>{title}</h2>
        {running && (
          <p role="status" className="hint">
            {running}
          </p>
        )}
        {error && <p role="alert">{error}</p>}
        {lines && (
          <ul className="maintenance-result">
            {lines.map((l, i) => (
              <li key={i}>{l}</li>
            ))}
          </ul>
        )}
        <div className="mask-buttons">
          {running ? (
            <button type="button" onClick={onCancel}>
              Stop
            </button>
          ) : (
            <button type="button" onClick={onClose}>
              Close
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
