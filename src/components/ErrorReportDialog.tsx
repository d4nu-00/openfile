import { useState } from "react";
import type {
  ContextRow,
  ErrorReport,
  ErrorReportParams,
  InstalledEngine,
  SelectionPlayer,
} from "../api/types";

const bands = ["winning", "better", "equal", "worse", "lost"];
const severities = [
  "Inaccuracies (25–49)",
  "Mistakes (50–99)",
  "Blunders (100–299)",
  "Losing blunders (300+)",
];
const phaseOrder = ["opening", "middlegame", "endgame"];
const opponentOrder = ["stronger", "equal", "weaker", "unrated"];
const fixed = (v?: number | null, digits = 1) =>
  v == null ? "" : v.toFixed(digits);

function ContextTable({
  label,
  rows,
  order,
}: {
  label: string;
  rows: Record<string, ContextRow>;
  order: string[];
}) {
  return (
    <section aria-label={label}>
      <h3>{label}</h3>
      <table className="stat-table">
        <thead>
          <tr>
            <th />
            <th className="num">Moves</th>
            <th className="num" title="Moves losing 0.50 or more">
              Errors
            </th>
            <th className="num">Av. loss</th>
          </tr>
        </thead>
        <tbody>
          {order
            .filter((k) => rows[k])
            .map((k) => (
              <tr key={k}>
                <th scope="row">{k[0].toUpperCase() + k.slice(1)}</th>
                <td className="num">{rows[k].moves}</td>
                <td className="num">{rows[k].errors}</td>
                <td className="num">{fixed(rows[k].averageLoss)}</td>
              </tr>
            ))}
        </tbody>
      </table>
    </section>
  );
}

/**
 * Error Report (the reference app's `BLUNDER_REPORT`, key B): an engine analyses
 * the selected games to the reference app's depth (Quick 9, Slower 12), and the
 * counted moves' losses give the error value, the reference app's Blunder Elo
 * curve and the workbench's blunder statistics.
 */
export function ErrorReportDialog({
  engines,
  defaultEngine,
  players,
  selected,
  report,
  running,
  error,
  onChooseFile,
  onStart,
  onCancel,
  onLoadGame,
  onClose,
}: {
  engines: InstalledEngine[];
  defaultEngine?: string;
  /** The selection's players, most games first. */
  players: SelectionPlayer[];
  /** Games selected. */
  selected: number;
  report?: ErrorReport;
  /** Progress while analysing. */
  running?: string;
  error?: string;
  onChooseFile: () => Promise<string | string[] | null>;
  onStart: (params: Omit<ErrorReportParams, "ranges">) => void;
  onCancel: () => void;
  onLoadGame: (ordinal: string, node: number) => void;
  onClose: () => void;
}) {
  const [engine, setEngine] = useState(
    engines.find((e) => e.id === defaultEngine)?.path ?? engines[0]?.path,
  );
  const [slower, setSlower] = useState(false);
  const [player, setPlayer] = useState<string>(
    players[0] ? String(players[0].id) : "both",
  );
  const options = engines.find((e) => e.path === engine)?.options ?? {};
  const r = report;
  const per100 = (n: number) =>
    r && r.moves ? ((n * 100) / r.moves).toFixed(1) : "";
  const peak = Math.max(1, ...(r?.distribution ?? [0]));
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Error Report"
        className="modal error-report"
        onSubmit={(e) => {
          e.preventDefault();
          if (engine && !running)
            onStart({
              engine,
              options,
              slower,
              player: player === "both" ? null : Number(player),
            });
        }}
        onKeyDown={(e) => e.key === "Escape" && !running && onClose()}
      >
        <h2>Error Report</h2>
        <div className="error-setup">
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
          <fieldset className="mask-row" disabled={!!running}>
            <legend className="mask-label">Analysis</legend>
            <label className="mask-check">
              <input
                type="radio"
                name="error-depth"
                checked={!slower}
                onChange={() => setSlower(false)}
              />
              Quick, normal games (depth 9)
            </label>
            <label className="mask-check">
              <input
                type="radio"
                name="error-depth"
                checked={slower}
                onChange={() => setSlower(true)}
              />
              Slower, top games (depth 12)
            </label>
          </fieldset>
          <label className="mask-row">
            <span>Moves of</span>{" "}
            <select
              aria-label="Moves of"
              value={player}
              disabled={!!running}
              onChange={(e) => setPlayer(e.target.value)}
            >
              {players.map((p) => (
                <option key={p.id} value={String(p.id)}>
                  {p.name ?? "?"} ({p.games} {p.games === 1 ? "game" : "games"})
                </option>
              ))}
              <option value="both">Both sides</option>
            </select>
          </label>
          <p className="hint">
            {selected} {selected === 1 ? "game" : "games"} selected; at most
            1,000 are analysed, up to move 60.
          </p>
        </div>
        {running && (
          <div className="empty-state">
            {running}{" "}
            <button type="button" className="link-button" onClick={onCancel}>
              Stop
            </button>
          </div>
        )}
        {error && <div className="empty-state">{error}</div>}
        {r && !running && (
          <div className="error-body">
            <section aria-label="Summary">
              <p className="error-elo">
                {r.blunderElo != null ? (
                  <>
                    Blunder Elo (estimate) = <strong>{r.blunderElo}</strong>
                    {r.difference != null && (
                      <span
                        className={r.difference > 0 ? "better" : "worse"}
                      >{` (${r.difference > 0 ? "+" : ""}${r.difference} against Elo ${r.rating})`}</span>
                    )}
                  </>
                ) : (
                  `Too few moves for a Blunder Elo (at least 20 are needed).`
                )}
              </p>
              <p className="hint">
                {r.player ? `${r.player}: ` : "Both sides: "}
                {r.games} games, {r.moves} moves counted at depth {r.depth}
                {r.skipped ? `, ${r.skipped} games skipped` : ""}
                {r.truncated ? ", the first 1,000 selected" : ""}
                {r.cancelled ? ", stopped" : ""}. Average loss{" "}
                {fixed(r.averageLoss)} cp, error value {fixed(r.errorValue)}
                {r.pliesPerGame != null
                  ? `, ${fixed(r.pliesPerGame, 0)} plies per game`
                  : ""}
                . The estimate follows a fixed rating curve and varies a lot
                with few games.
              </p>
            </section>
            <div className="error-columns">
              <section aria-label="Severity">
                <h3>Severity</h3>
                <table className="stat-table">
                  <thead>
                    <tr>
                      <th />
                      <th className="num">N</th>
                      <th className="num">Per 100 moves</th>
                    </tr>
                  </thead>
                  <tbody>
                    {severities.map((label, i) => (
                      <tr key={label}>
                        <th scope="row">{label}</th>
                        <td className="num">{r.severity[i]}</td>
                        <td className="num">{per100(r.severity[i])}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </section>
              <section aria-label="Distribution">
                <h3>Losses by size</h3>
                <div className="error-bars">
                  {r.distribution.map((n, i) => (
                    <div
                      key={i}
                      className="error-bar"
                      title={`${25 * (i + 1)}–${25 * (i + 2) - 1}: ${n}`}
                    >
                      <span style={{ height: `${(n * 100) / peak}%` }} />
                    </div>
                  ))}
                </div>
                <p className="hint">0.25 to 4.00 pawns, in steps of 0.25.</p>
              </section>
            </div>
            <div className="error-columns">
              <ContextTable
                label="By phase"
                rows={r.phases}
                order={phaseOrder}
              />
              <ContextTable
                label="By opponent"
                rows={r.opponents}
                order={opponentOrder}
              />
            </div>
            <section aria-label="Evaluation before and after">
              <h3>Errors by evaluation before (rows) and after</h3>
              <table className="stat-table error-matrix">
                <thead>
                  <tr>
                    <th />
                    {bands.map((b) => (
                      <th key={b} className="num">
                        {b}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {bands.map((b, i) => (
                    <tr key={b}>
                      <th scope="row">{b}</th>
                      {bands.map((c, j) => (
                        <td key={c} className="num">
                          {j >= i ? r.transitions[i][j] : ""}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
            <section aria-label="Worst moves">
              <h3>Worst moves</h3>
              <table className="stat-table">
                <thead>
                  <tr>
                    <th className="num">Game</th>
                    <th>Move</th>
                    <th>Best</th>
                    <th className="num">Loss</th>
                  </tr>
                </thead>
                <tbody>
                  {r.worst.map((m) => (
                    <tr
                      key={`${m.ordinal}-${m.node}`}
                      title="Double-click to load the game at this move"
                      onDoubleClick={() => onLoadGame(m.ordinal, m.node)}
                    >
                      <td className="num">{m.ordinal}</td>
                      <td>
                        {m.number} {m.san}
                      </td>
                      <td>{m.best ?? ""}</td>
                      <td className="num">{(m.loss / 100).toFixed(2)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          </div>
        )}
        <div className="modal-actions">
          <button type="submit" disabled={!engine || !!running}>
            {r ? "Run again" : "Start"}
          </button>
          <button type="button" onClick={onClose} disabled={!!running}>
            Close
          </button>
        </div>
      </form>
    </div>
  );
}
