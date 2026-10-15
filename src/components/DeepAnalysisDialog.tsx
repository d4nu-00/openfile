import { useState } from "react";
import type { DeepParams, DeepProgress, InstalledEngine } from "../api/types";

/** Elapsed time as m:ss or h:mm:ss. */
export function clock(ms: number) {
  const s = Math.floor(ms / 1000);
  const two = (n: number) => String(n).padStart(2, "0");
  return s >= 3600
    ? `${Math.floor(s / 3600)}:${two(Math.floor(s / 60) % 60)}:${two(s % 60)}`
    : `${Math.floor(s / 60)}:${two(s % 60)}`;
}

/**
 * Deep Analysis: ChessBase's iterated tree analysis of the board position.
 * The tree grows each iteration until stopped or out of time; Stop keeps
 * it and adds it to the game as one edit, Cancel discards it.
 */
export function DeepAnalysisDialog({
  engines,
  defaultEngine,
  running,
  onChooseFile,
  onStart,
  onStop,
  onClose,
  error,
}: {
  engines: InstalledEngine[];
  defaultEngine?: string;
  /** Progress while running; `true` before the first report. */
  running?: DeepProgress | true;
  onChooseFile: () => Promise<string | string[] | null>;
  onStart: (params: Omit<DeepParams, "node">) => void;
  /** Stops the analysis; `discard` drops what it found. */
  onStop: (discard: boolean) => void;
  onClose: () => void;
  error?: string;
}) {
  const [engine, setEngine] = useState(
    engines.find((e) => e.id === defaultEngine)?.path ?? engines[0]?.path,
  );
  const [limited, setLimited] = useState(false);
  const [minutes, setMinutes] = useState("10");
  const m = Number(minutes);
  const valid =
    !!engine && (!limited || (Number.isInteger(m) && m >= 1 && m <= 1440));
  const options = engines.find((e) => e.path === engine)?.options ?? {};
  const p = running === true ? undefined : running;
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Deep Analysis"
        className="modal deep-analysis"
        onSubmit={(e) => {
          e.preventDefault();
          if (valid && engine && !running)
            onStart({
              engine,
              options,
              timeLimitSecs: limited ? m * 60 : 0,
            });
        }}
        onKeyDown={(e) => e.key === "Escape" && !running && onClose()}
      >
        <h2>Deep Analysis</h2>
        <label className="mask-row">
          <span>Engine</span>{" "}
          <select
            aria-label="Engine"
            value={engine ?? ""}
            disabled={!!running}
            onChange={(e) => setEngine(e.target.value)}
          >
            {!engine && <option value="">Choose an engine</option>}
            {engines.map((e) => (
              <option key={e.id} value={e.path}>
                {e.name}
              </option>
            ))}
            {engine && !engines.some((e) => e.path === engine) && (
              <option value={engine}>{engine}</option>
            )}
          </select>{" "}
          <button
            type="button"
            disabled={!!running}
            onClick={() =>
              void onChooseFile().then((f) => {
                if (typeof f === "string") setEngine(f);
              })
            }
          >
            Other engine…
          </button>
        </label>
        <fieldset className="mask-row" disabled={!!running}>
          <legend className="mask-label">Duration</legend>
          <label className="mask-check">
            <input
              type="radio"
              name="deep-duration"
              checked={!limited}
              onChange={() => setLimited(false)}
            />
            Until stopped
          </label>
          <label className="mask-check">
            <input
              type="radio"
              name="deep-duration"
              checked={limited}
              onChange={() => setLimited(true)}
            />
            Minutes
          </label>{" "}
          <input
            type="number"
            aria-label="Minutes"
            min={1}
            max={1440}
            step={1}
            value={minutes}
            disabled={!limited}
            onChange={(e) => setMinutes(e.target.value)}
          />
        </fieldset>
        <p className="hint">
          The engine analyses the board position with several lines and follows
          those within 0.70 of the best, wider and deeper each iteration (4 s
          per position at first, growing by 1.7 each time). The tree goes into
          the game when the analysis ends; Undo removes it.
        </p>
        {running && (
          <div role="status" className="hint deep-progress">
            {p ? (
              <>
                <div>
                  Iteration {p.iteration} · {p.positions} positions · {p.nodes}{" "}
                  moves · {clock(p.elapsedMs)}
                </div>
                {p.best.length > 0 && (
                  <div className="deep-best">
                    {p.best.join(" ")}
                    {p.score ? ` (${p.score})` : ""}
                  </div>
                )}
              </>
            ) : (
              "Starting…"
            )}
          </div>
        )}
        {error && <p role="alert">{error}</p>}
        <div className="mask-buttons">
          {running ? (
            <>
              <button type="button" onClick={() => onStop(false)}>
                Stop
              </button>
              <button type="button" onClick={() => onStop(true)}>
                Cancel
              </button>
            </>
          ) : (
            <>
              <button type="submit" disabled={!valid}>
                Start
              </button>
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
