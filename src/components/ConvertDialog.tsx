import type { ConvertEstimate } from "../api/types";

export type ConvertScope = "all" | "selection";

const gb = (bytes?: string | null) =>
  bytes == null ? "unknown" : `${(Number(bytes) / 1e9).toFixed(2)} GB`;

/**
 * Convert to 2CBH (ChessBase's "Convert to free 2CBH format"): writes the
 * whole database, or the selected games, into a new 2CBH database. The
 * source is only read; the estimate must fit the target disk.
 */
export function ConvertDialog({
  name,
  entries,
  selected,
  scope,
  target,
  estimate,
  running,
  error,
  onScope,
  onTarget,
  onStart,
  onCancel,
  onClose,
}: {
  name: string;
  entries: string;
  /** Games selected in the list; the choice is offered above one. */
  selected: number;
  scope: ConvertScope;
  target: string;
  estimate?: ConvertEstimate;
  /** Progress while converting, e.g. "1,200 of 50,000 entries". */
  running?: string;
  error?: string;
  onScope: (scope: ConvertScope) => void;
  onTarget: () => void;
  onStart: () => void;
  onCancel: () => void;
  onClose: () => void;
}) {
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Convert to 2CBH"
        className="modal convert"
        onSubmit={(e) => {
          e.preventDefault();
          if (estimate?.fits && !running) onStart();
        }}
        onKeyDown={(e) => e.key === "Escape" && !running && onClose()}
      >
        <h2>Convert to free 2CBH format</h2>
        <p>
          <strong>{name}</strong> is only read; the games are written into a new
          database.
        </p>
        <fieldset className="mask-row" disabled={!!running}>
          <legend className="mask-label">Games</legend>
          <label className="mask-check">
            <input
              type="radio"
              name="convert-scope"
              checked={scope === "all"}
              onChange={() => onScope("all")}
            />
            Whole database ({entries} entries)
          </label>
          {selected > 1 && (
            <label className="mask-check">
              <input
                type="radio"
                name="convert-scope"
                checked={scope === "selection"}
                onChange={() => onScope("selection")}
              />
              Selected games ({selected})
            </label>
          )}
        </fieldset>
        <div className="mask-row">
          <span className="mask-label">New database</span>
          <span className="path" title={target}>
            {target}
          </span>
          <button type="button" disabled={!!running} onClick={onTarget}>
            Change…
          </button>
        </div>
        <p className="hint" role="note">
          {estimate
            ? `About ${gb(estimate.bytes)} needed; ${gb(estimate.free)} free on that disk.`
            : "Estimating the space needed…"}
        </p>
        {estimate && !estimate.fits && (
          <p role="alert">
            Not enough disk space: 256 MB must stay free beyond the estimate.
            Choose another disk or fewer games.
          </p>
        )}
        <p className="hint">
          Annotations that are not decoded, or that 2CBH cannot store, are left
          out, as when games are pasted. Text entries and Chess960 games are
          left out and counted. Deleted games stay marked deleted.
        </p>
        {running && (
          <p role="status" className="hint">
            Converting: {running}
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
              <button type="submit" disabled={!estimate?.fits}>
                Convert
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
