import { useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type {
  CentipawnReport,
  CentipawnSide,
  InstalledEngine,
} from "../api/types";

/** The reference app's categories, in its order (`0x140418b10`). */
const categories = [
  "Brilliant move",
  "Strong move",
  "Engine's move",
  "Good alternative",
  "Inaccuracy",
  "Mistake",
  "Lets the win slip",
  "Losing move",
];

const value = (v: number | null | undefined, digits: number) =>
  v == null ? "–" : v.toFixed(digits);

/**
 * Centipawn Analysis (the reference app's `CENTIPAWN`, Ctrl+Alt+Shift+Z): an
 * engine checks the main line's moves after the opening; the average loss,
 * the error value and each move's category go into the game as texts, as
 * one edit.
 */
export function CentipawnDialog({
  api,
  documentId,
  engines,
  defaultEngine,
  onChooseFile,
  onApply,
  onClose,
}: {
  api: WorkbenchApi;
  documentId: string;
  engines: InstalledEngine[];
  defaultEngine?: string;
  onChooseFile: () => Promise<string | string[] | null>;
  /** Adds the report's texts to the game; resolves to an error, if any. */
  onApply: (report: CentipawnReport) => Promise<string | undefined>;
  onClose: () => void;
}) {
  const [engine, setEngine] = useState(
    engines.find((e) => e.id === defaultEngine)?.path ?? engines[0]?.path,
  );
  const [seconds, setSeconds] = useState("1");
  const [everyMove, setEveryMove] = useState(false);
  const [running, setRunning] = useState<string>();
  const [error, setError] = useState<string>();
  const [report, setReport] = useState<CentipawnReport>();
  const busy = useRef(false);
  const time = Number(seconds);
  const valid = !!engine && Number.isInteger(time) && time >= 0 && time <= 60;
  const options = engines.find((e) => e.path === engine)?.options ?? {};

  async function start() {
    if (!engine || busy.current) return;
    busy.current = true;
    setError(undefined);
    setReport(undefined);
    setRunning("starting…");
    try {
      const r = await api.centipawnAnalysis(
        documentId,
        { engine, options, seconds: time, everyMove },
        (p) => setRunning(`${p.done} of ${p.total} positions`),
      );
      if (r.cancelled) setError("Stopped; nothing was added.");
      else {
        const failed = await onApply(r);
        if (failed) setError(failed);
        else setReport(r);
      }
    } catch (e) {
      setError((e as { message?: string })?.message ?? String(e));
    } finally {
      busy.current = false;
      setRunning(undefined);
    }
  }

  const side = (name: string, s: CentipawnSide) => (
    <tr key={name}>
      <th scope="row">{name}</th>
      <td className="num">{s.moves}</td>
      <td className="num strong">{value(s.averageLoss, 0)}</td>
      <td className="num">{value(s.errorValue, 2)}</td>
    </tr>
  );
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Centipawn Analysis"
        className="modal centipawn"
        onSubmit={(e) => {
          e.preventDefault();
          if (valid) void start();
        }}
        onKeyDown={(e) => e.key === "Escape" && !running && onClose()}
      >
        <h2>Centipawn Analysis</h2>
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
          <span>Seconds</span>{" "}
          <input
            type="number"
            aria-label="Seconds"
            min={0}
            max={60}
            step={1}
            value={seconds}
            disabled={!!running}
            onChange={(e) => setSeconds(e.target.value)}
          />
          <span className="hint">
            {" "}
            {valid
              ? `${time === 0 ? 0.2 : Math.max(0.5, time * 0.75)} s per position`
              : "0 to 60"}
          </span>
        </label>
        <label className="mask-check">
          <input
            type="checkbox"
            checked={everyMove}
            disabled={!!running}
            onChange={(e) => setEveryMove(e.target.checked)}
          />
          Comment every classified move, not only the errors
        </label>
        <p className="hint">
          The main line is analysed after its opening (the opening table's
          moves, at least ten plies). The summary goes before the first move;
          Undo removes all of it.
        </p>
        {running && (
          <p role="status" className="hint">
            Analysing: {running}
          </p>
        )}
        {error && <p role="alert">{error}</p>}
        {report && (
          <div className="centipawn-result">
            <p className="hint">
              After ply {report.bookPlies}, {report.positions} positions.
            </p>
            <table className="stat-table" aria-label="Centipawn loss">
              <thead>
                <tr>
                  <th />
                  <th className="num">Moves</th>
                  <th className="num" title="Average loss in centipawns">
                    Loss
                  </th>
                  <th
                    className="num"
                    title="The Error Report's weighted error value"
                  >
                    Error
                  </th>
                </tr>
              </thead>
              <tbody>
                {side("White", report.white)}
                {side("Black", report.black)}
              </tbody>
            </table>
            <table className="stat-table" aria-label="Moves by category">
              <thead>
                <tr>
                  <th>Category</th>
                  <th className="num">White</th>
                  <th className="num">Black</th>
                </tr>
              </thead>
              <tbody>
                {categories.map((name, i) =>
                  report.white.categories[i] + report.black.categories[i] ? (
                    <tr key={name}>
                      <td>{name}</td>
                      <td className="num">{report.white.categories[i]}</td>
                      <td className="num">{report.black.categories[i]}</td>
                    </tr>
                  ) : null,
                )}
              </tbody>
            </table>
          </div>
        )}
        <div className="mask-buttons">
          {running ? (
            <button
              type="button"
              onClick={() =>
                void api.cancelCentipawnAnalysis(documentId).catch(() => {})
              }
            >
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
