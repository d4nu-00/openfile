import { useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type { CentipawnListReport, InstalledEngine } from "../api/types";

/** Players listed in the dialog at most; the CSV has them all. */
const SHOWN_PLAYERS = 200;

/**
 * Centipawn Analysis of the selected games of a 2CBH database opened for
 * editing: each game is annotated and replaces its entry, and the players'
 * statistics can be saved as a CSV file.
 */
export function CentipawnListDialog({
  api,
  databaseId,
  databaseName,
  ranges,
  selected,
  engines,
  defaultEngine,
  onChooseFile,
  onDone,
  onClose,
}: {
  api: WorkbenchApi;
  databaseId: string;
  databaseName: string;
  ranges: { start: string; end: string }[];
  /** Games chosen. */
  selected: number;
  engines: InstalledEngine[];
  defaultEngine?: string;
  onChooseFile: () => Promise<string | string[] | null>;
  /** The list changed: refresh it. */
  onDone: () => void;
  onClose: () => void;
}) {
  const [engine, setEngine] = useState(
    engines.find((e) => e.id === defaultEngine)?.path ?? engines[0]?.path,
  );
  const [seconds, setSeconds] = useState("1");
  const [everyMove, setEveryMove] = useState(false);
  const [running, setRunning] = useState<string>();
  const [error, setError] = useState<string>();
  const [report, setReport] = useState<CentipawnListReport>();
  const [saved, setSaved] = useState<string>();
  const busy = useRef(false);
  const time = Number(seconds);
  const valid =
    !!engine &&
    Number.isInteger(time) &&
    time >= 0 &&
    time <= 60 &&
    selected > 0;
  const options = engines.find((e) => e.path === engine)?.options ?? {};

  async function start() {
    if (!engine || busy.current) return;
    busy.current = true;
    setError(undefined);
    setReport(undefined);
    setSaved(undefined);
    setRunning("starting…");
    try {
      const r = await api.centipawnList(
        databaseId,
        ranges,
        { engine, options, seconds: time, everyMove },
        (p) =>
          setRunning(
            `Game ${p.game} of ${p.games}: ${p.label}${p.total ? ` (${p.done} of ${p.total} positions)` : ""}`,
          ),
      );
      setReport(r);
      if (r.error) setError(r.error);
      else if (r.cancelled)
        setError("Stopped; the games analysed so far were replaced.");
      if (r.replaced) onDone();
    } catch (e) {
      setError((e as { message?: string })?.message ?? String(e));
    } finally {
      busy.current = false;
      setRunning(undefined);
    }
  }

  async function exportCsv() {
    if (!report) return;
    const path = await api.chooseCsv(`${databaseName}-CentiPawnLoss.csv`);
    if (typeof path !== "string") return;
    try {
      await api.saveCsv(path, report.csv);
      setSaved(`Saved ${path}`);
    } catch (e) {
      setError((e as { message?: string })?.message ?? String(e));
    }
  }

  const num = (v: number | null | undefined, digits: number) =>
    v == null ? "–" : v.toFixed(digits);
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Centipawn Analysis of the selected games"
        className="modal centipawn-list"
        onSubmit={(e) => {
          e.preventDefault();
          if (valid) void start();
        }}
        onKeyDown={(e) => e.key === "Escape" && !running && onClose()}
      >
        <h2>Centipawn Analysis of {selected} games</h2>
        <p className="hint">
          Each game is analysed and annotated, and replaces its entry in the
          database.
        </p>
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
            value={seconds}
            disabled={!!running}
            onChange={(e) => setSeconds(e.target.value)}
          />
        </label>
        <label className="mask-row">
          <input
            type="checkbox"
            checked={everyMove}
            disabled={!!running}
            onChange={(e) => setEveryMove(e.target.checked)}
          />{" "}
          Comment every classified move
        </label>
        {running && <p role="status">{running}</p>}
        {error && <p role="alert">{error}</p>}
        {report && (
          <section aria-label="Centipawn statistics">
            <p>
              {report.analysed} analysed, {report.replaced} replaced
              {report.skippedCount ? `, ${report.skippedCount} skipped` : ""}.
            </p>
            {report.players.length > 0 && (
              <div className="table-scroll">
                <table aria-label="Players">
                  <thead>
                    <tr>
                      <th>Player</th>
                      <th className="num">Elo</th>
                      <th className="num">Perform</th>
                      <th className="num">Points</th>
                      <th className="num">Games</th>
                      <th className="num">Weighted av.</th>
                    </tr>
                  </thead>
                  <tbody>
                    {report.players.slice(0, SHOWN_PLAYERS).map((p) => (
                      <tr key={p.name}>
                        <th scope="row">{p.name}</th>
                        <td className="num">{p.elo || "–"}</td>
                        <td className="num">{p.perform ?? "–"}</td>
                        <td className="num">{p.points.toFixed(1)}</td>
                        <td className="num">{p.games.length}</td>
                        <td className="num strong">
                          {num(p.weightedAverage, 2)}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
            {report.skipped.length > 0 && (
              <details>
                <summary>Skipped games</summary>
                <ul>
                  {report.skipped.map((s) => (
                    <li key={s.ordinal}>
                      Game {s.ordinal}: {s.reason}
                    </li>
                  ))}
                </ul>
              </details>
            )}
            {saved && <p role="status">{saved}</p>}
          </section>
        )}
        <div className="modal-actions">
          {report && report.players.length > 0 && (
            <button type="button" onClick={() => void exportCsv()}>
              Save CSV…
            </button>
          )}
          {running ? (
            <button
              type="button"
              onClick={() => void api.cancelCentipawnList()}
            >
              Stop
            </button>
          ) : (
            <button type="submit" disabled={!valid}>
              Analyse
            </button>
          )}
          <button type="button" disabled={!!running} onClick={onClose}>
            Close
          </button>
        </div>
      </form>
    </div>
  );
}
