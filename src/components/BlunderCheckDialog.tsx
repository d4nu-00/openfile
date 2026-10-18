import { useState } from "react";
import type { BlunderParams, InstalledEngine } from "../api/types";

/** the reference app's thresholds, in centipawns (`0x14042bf80`). */
export const thresholds = { fine: 35, medium: 55, coarse: 150 } as const;

/**
 * Tactical Analysis (Blunder Check): an engine checks every move of the
 * game's main line for the side chosen; moves that lose at least the
 * threshold get a symbol and the engine's line, and optionally their
 * evaluation.
 */
export function BlunderCheckDialog({
  engines,
  defaultEngine,
  running,
  onChooseFile,
  onStart,
  onCancel,
  onClose,
  error,
}: {
  engines: InstalledEngine[];
  defaultEngine?: string;
  /** Progress while checking, e.g. "12 of 80 positions". */
  running?: string;
  onChooseFile: () => Promise<string | string[] | null>;
  onStart: (params: BlunderParams, storeEvaluations: boolean) => void;
  onCancel: () => void;
  onClose: () => void;
  error?: string;
}) {
  const [engine, setEngine] = useState(
    engines.find((e) => e.id === defaultEngine)?.path ?? engines[0]?.path,
  );
  const [seconds, setSeconds] = useState("3");
  const [grain, setGrain] = useState<keyof typeof thresholds>("fine");
  const [side, setSide] = useState<BlunderParams["side"]>("both");
  const [store, setStore] = useState(false);
  const time = Number(seconds);
  const valid = !!engine && Number.isInteger(time) && time >= 1 && time <= 120;
  const options = engines.find((e) => e.path === engine)?.options ?? {};
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Tactical Analysis"
        className="modal blunder-check"
        onSubmit={(e) => {
          e.preventDefault();
          if (valid && engine && !running)
            // As the reference app: Time × 500 ms for each position.
            onStart(
              {
                engine,
                options,
                millisPerMove: time * 500,
                threshold: thresholds[grain],
                side,
              },
              store,
            );
        }}
        onKeyDown={(e) => e.key === "Escape" && !running && onClose()}
      >
        <h2>Tactical Analysis</h2>
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
              void onChooseFile().then((p) => {
                if (typeof p === "string") setEngine(p);
              })
            }
          >
            Other engine…
          </button>
        </label>
        <label className="mask-row">
          <span>Time [s]</span>{" "}
          <input
            type="number"
            aria-label="Time"
            min={1}
            max={120}
            step={1}
            value={seconds}
            disabled={!!running}
            onChange={(e) => setSeconds(e.target.value)}
          />
        </label>
        <fieldset className="mask-row" disabled={!!running}>
          <legend className="mask-label">Threshold</legend>
          {(
            [
              ["fine", "Fine"],
              ["medium", "Medium"],
              ["coarse", "Coarse"],
            ] as const
          ).map(([value, label]) => (
            <label key={value} className="mask-check">
              <input
                type="radio"
                name="blunder-threshold"
                checked={grain === value}
                onChange={() => setGrain(value)}
              />
              {label} ({(thresholds[value] / 100).toFixed(2)})
            </label>
          ))}
        </fieldset>
        <fieldset className="mask-row" disabled={!!running}>
          <legend className="mask-label">Moves of</legend>
          {(
            [
              ["both", "Both sides"],
              ["white", "White"],
              ["black", "Black"],
            ] as const
          ).map(([value, label]) => (
            <label key={value} className="mask-check">
              <input
                type="radio"
                name="blunder-side"
                checked={side === value}
                onChange={() => setSide(value)}
              />
              {label}
            </label>
          ))}
        </fieldset>
        <label className="mask-check">
          <input
            type="checkbox"
            checked={store}
            disabled={!!running}
            onChange={(e) => setStore(e.target.checked)}
          />
          Store evaluations
        </label>
        <p className="hint">
          A move that loses at least the threshold (in pawns) gets ? (?? from
          three times the threshold) and the engine's line as a variation. The
          main line is checked; Undo removes all of it.
        </p>
        {running && (
          <p role="status" className="hint">
            Checking: {running}
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
