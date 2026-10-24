import { useState } from "react";
import type {
  InstalledEngine,
  MateBuildOptions,
  MateTrainingProgress,
  MateTrainingResponse,
} from "../api/types";
import { parseCombinations, rejectionLabels } from "../state/mateTraining";

export interface MateTrainingRequest {
  source: "all" | "selection";
  options: MateBuildOptions;
  /** Empty when the depths stop at 2: those mates are solved exactly. */
  engineId: string;
  millis: number;
  folder: string;
  name: string;
}

const depths = [1, 2, 3, 4, 5];
const baseName = (path: string) => path.split(/[\\/]/).at(-1) ?? path;

/**
 * Build Mate Training: training databases of mates in 1 to 5 from the games in
 * the list, balanced across the combinations of pieces in the mate. Mates in
 * 1 and 2 are solved exactly; deeper ones are checked with an engine.
 */
export function MateTrainingDialog({
  games,
  selected,
  engines,
  defaultEngine,
  running,
  result,
  error,
  onChooseFolder,
  onBuild,
  onStop,
  onCancel,
  onOpen,
  onClose,
}: {
  /** Games in the list, and the rows selected in it. */
  games: number;
  selected: number;
  engines: InstalledEngine[];
  defaultEngine?: string;
  /** Set while building. */
  running?: MateTrainingProgress;
  result?: MateTrainingResponse;
  error?: string;
  onChooseFolder: () => Promise<string | string[] | null>;
  onBuild: (request: MateTrainingRequest) => void;
  onStop: () => void;
  onCancel: () => void;
  onOpen: (path: string) => void;
  onClose: () => void;
}) {
  const [source, setSource] = useState<"all" | "selection">(
    selected > 1 ? "selection" : "all",
  );
  const [from, setFrom] = useState(1);
  const [to, setTo] = useState(5);
  const [per, setPer] = useState("20");
  const [only, setOnly] = useState(false);
  const [list, setList] = useState("");
  const [pool, setPool] = useState(false);
  const [engineId, setEngineId] = useState(
    engines.find((e) => e.id === defaultEngine)?.id ?? engines[0]?.id ?? "",
  );
  const [millis, setMillis] = useState("1000");
  const [folder, setFolder] = useState("");
  const [name, setName] = useState("Mate training");
  const busy = !!running;
  const perCount = Number(per);
  const time = Number(millis);
  const parsed = only ? parseCombinations(list, pool) : { combinations: [] };
  const invalid = "invalid" in parsed ? parsed.invalid : undefined;
  const needsEngine = to >= 3;
  const valid =
    from <= to &&
    Number.isInteger(perCount) &&
    perCount >= 1 &&
    perCount <= 1000 &&
    Number.isInteger(time) &&
    time >= 100 &&
    time <= 60000 &&
    !invalid &&
    (!only || ("combinations" in parsed && parsed.combinations.length > 0)) &&
    folder !== "" &&
    name.trim() !== "" &&
    (!needsEngine || engineId !== "");
  const submit = () => {
    if (!valid || busy || !("combinations" in parsed)) return;
    onBuild({
      source,
      options: {
        minDepth: from,
        maxDepth: to,
        per: perCount,
        combinations: only ? parsed.combinations : null,
        poolMinors: pool,
      },
      engineId: needsEngine ? engineId : "",
      millis: time,
      folder,
      name: name.trim(),
    });
  };
  const total = (n: number[], d: number) => n[d - 1] ?? 0;
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Build Mate Training"
        className="modal mate-training"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
        onKeyDown={(e) => e.key === "Escape" && !busy && onClose()}
      >
        <h2>Build Mate Training</h2>
        <fieldset className="mask-row" disabled={busy}>
          <legend className="mask-label">Source</legend>
          <label className="mask-check">
            <input
              type="radio"
              name="mate-source"
              checked={source === "all"}
              onChange={() => setSource("all")}
            />
            All games in the list ({games.toLocaleString()})
          </label>
          <label className="mask-check">
            <input
              type="radio"
              name="mate-source"
              checked={source === "selection"}
              disabled={selected === 0}
              onChange={() => setSource("selection")}
            />
            Selected games ({selected.toLocaleString()})
          </label>
        </fieldset>
        <div className="mask-row">
          <label>
            From depth{" "}
            <select
              aria-label="From depth"
              value={from}
              disabled={busy}
              onChange={(e) => setFrom(Number(e.target.value))}
            >
              {depths.map((d) => (
                <option key={d} value={d}>
                  {d}
                </option>
              ))}
            </select>
          </label>
          <label>
            To depth{" "}
            <select
              aria-label="To depth"
              value={to}
              disabled={busy}
              onChange={(e) => setTo(Number(e.target.value))}
            >
              {depths.map((d) => (
                <option key={d} value={d}>
                  {d}
                </option>
              ))}
            </select>
          </label>
          <label>
            Puzzles per combination{" "}
            <input
              type="number"
              aria-label="Puzzles per combination"
              min={1}
              max={1000}
              value={per}
              disabled={busy}
              onChange={(e) => setPer(e.target.value)}
            />
          </label>
        </div>
        <fieldset className="mask-row" disabled={busy}>
          <legend className="mask-label">Combinations</legend>
          <label className="mask-check">
            <input
              type="radio"
              name="mate-combinations"
              checked={!only}
              onChange={() => setOnly(false)}
            />
            Every combination found
          </label>
          <label className="mask-check">
            <input
              type="radio"
              name="mate-combinations"
              checked={only}
              onChange={() => setOnly(true)}
            />
            Only these
          </label>
          <input
            aria-label="Combination list"
            placeholder="Q+N, R+K, B+N"
            value={list}
            disabled={!only}
            onChange={(e) => setList(e.target.value)}
          />
        </fieldset>
        {invalid && (
          <p role="alert">
            "{invalid}" is not a combination such as Q+N. Pieces are Q, R, B, N,
            P and K{pool ? ", and m for a bishop or knight" : ""}, joined by +.
          </p>
        )}
        <label className="mask-check">
          <input
            type="checkbox"
            checked={pool}
            disabled={busy}
            onChange={(e) => setPool(e.target.checked)}
          />
          Bishop = Knight
        </label>
        <div className="mask-row">
          <label>
            Engine{" "}
            <select
              aria-label="Engine"
              value={engineId}
              disabled={busy || engines.length === 0}
              onChange={(e) => setEngineId(e.target.value)}
            >
              {engines.length === 0 && <option value="">No engine</option>}
              {engines.map((e) => (
                <option key={e.id} value={e.id}>
                  {e.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            Time per check (ms){" "}
            <input
              type="number"
              aria-label="Time per check (ms)"
              min={100}
              max={60000}
              step={100}
              value={millis}
              disabled={busy}
              onChange={(e) => setMillis(e.target.value)}
            />
          </label>
        </div>
        {needsEngine && engines.length === 0 && (
          <p className="hint">
            Mates in 3 to 5 need an engine: add one with Add Kibitzer, or stop
            at depth 2.
          </p>
        )}
        <div className="mask-row">
          <span className="mask-label">Folder</span>
          <span className="path" title={folder}>
            {folder}
          </span>
          <button
            type="button"
            disabled={busy}
            onClick={() =>
              void onChooseFolder().then((chosen) => {
                if (typeof chosen === "string") setFolder(chosen);
              })
            }
          >
            Choose…
          </button>
        </div>
        <label className="mask-row">
          <span className="mask-label">Name</span>
          <input
            aria-label="Name"
            value={name}
            disabled={busy}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <p className="hint">
          Mates in 1 and 2 are solved exactly; 3 to 5 are checked with the
          engine. Only positions with a single mating move are kept.
        </p>
        {running && (
          <div role="status" className="hint">
            <div>
              Building: {Number(running.scanned).toLocaleString()} of{" "}
              {Number(running.total).toLocaleString()} games
            </div>
            <div>
              {depths
                .filter((d) => d >= from && d <= to)
                .map((d) => `Mate in ${d}: ${total(running.puzzles, d)}`)
                .join(" · ")}
              {" · "}
              {running.chains} {running.chains === 1 ? "chain" : "chains"}
            </div>
          </div>
        )}
        {error && <p role="alert">{error}</p>}
        {result && (
          <section aria-label="Result">
            <p>
              {result.cancelled
                ? "Cancelled; nothing was written."
                : `${result.stopped ? "Stopped" : "Done"} after ${Number(result.scanned).toLocaleString()} of ${Number(result.total).toLocaleString()} games${result.complete ? " (every quota met)" : ""}.`}
            </p>
            {!result.cancelled && (
              <>
                <table
                  className="stat-table"
                  aria-label="Puzzles by combination"
                >
                  <thead>
                    <tr>
                      <th>Combination</th>
                      {depths
                        .filter((d) => d >= from && d <= to)
                        .map((d) => (
                          <th key={d} className="num">
                            Mate in {d}
                          </th>
                        ))}
                      <th className="num">Chains</th>
                    </tr>
                  </thead>
                  <tbody>
                    {result.combinations.map((c) => (
                      <tr key={c.combination}>
                        <th scope="row">{c.combination}</th>
                        {depths
                          .filter((d) => d >= from && d <= to)
                          .map((d) => (
                            <td key={d} className="num">
                              {total(c.depths, d)}
                            </td>
                          ))}
                        <td className="num">{c.chains}</td>
                      </tr>
                    ))}
                    <tr>
                      <th scope="row">Total</th>
                      {depths
                        .filter((d) => d >= from && d <= to)
                        .map((d) => (
                          <td key={d} className="num strong">
                            {total(result.puzzles, d)}
                          </td>
                        ))}
                      <td className="num strong">{result.chains}</td>
                    </tr>
                  </tbody>
                </table>
                <p>
                  {result.chains} {result.chains === 1 ? "chain" : "chains"} of
                  two or more steps.
                </p>
                {result.rejections.length > 0 && (
                  <>
                    <p>Not used:</p>
                    <ul className="mate-rejections">
                      {result.rejections.map((r) => (
                        <li key={r.kind}>
                          {rejectionLabels[r.kind] ?? r.kind}: {r.count}
                        </li>
                      ))}
                    </ul>
                  </>
                )}
                {result.databases.length === 0 ? (
                  <p>No puzzles were found, so no database was written.</p>
                ) : (
                  <>
                    <p>Written:</p>
                    <ul className="mate-databases">
                      {result.databases.map((d) => (
                        <li key={d.path}>
                          <span className="path" title={d.path}>
                            {d.path}
                          </span>{" "}
                          ({d.games} games){" "}
                          <button
                            type="button"
                            aria-label={`Open ${baseName(d.path)}`}
                            onClick={() => onOpen(d.path)}
                          >
                            Open
                          </button>
                        </li>
                      ))}
                    </ul>
                  </>
                )}
              </>
            )}
          </section>
        )}
        <div className="mask-buttons">
          {running ? (
            <>
              <button type="button" onClick={onStop}>
                Stop
              </button>
              <button type="button" onClick={onCancel}>
                Cancel
              </button>
            </>
          ) : (
            <>
              <button type="submit" disabled={!valid}>
                Build
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
