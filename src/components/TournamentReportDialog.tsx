import { useState } from "react";
import type { ReportRound, TournamentReport } from "../api/types";

/** A stored date without its unknown parts: "1905.??.??" is "1905". */
const dateText = (d: string) => d.replace(/(\.\?\?)+$/, "");
const points = (p: number) =>
  Number.isInteger(p) ? String(p) : `${Math.floor(p) || ""}½`;

/**
 * Tournament report (the reference app's `TournamentReport`): the tournament's
 * games round by round, each round with its date, as the reference app writes it,
 * with its options (a range of rounds and two columns). The standings
 * after each round and the result statistics are the workbench's additions.
 */
export function TournamentReportDialog({
  report,
  loading,
  error,
  onCrosstable,
  onListGames,
  onLoadGame,
  onClose,
}: {
  report?: TournamentReport;
  loading?: boolean;
  error?: string;
  onCrosstable: () => void;
  onListGames: (ordinals: string[], label: string) => void;
  onLoadGame: (ordinal: string) => void;
  onClose: () => void;
}) {
  // Kept as typed; empty means no limit.
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [twoColumns, setTwoColumns] = useState(false);
  const [standings, setStandings] = useState(true);
  const table = report?.table;
  const name = (row: number) => table?.players[row]?.name ?? "?";
  const first = Number(from) || 1;
  const last = Number(to) || Infinity;
  const shown = (r: ReportRound) =>
    r.round === 0 || (r.round >= first && r.round <= last);
  const title = table?.event ?? "Tournament";
  const s = report?.stats;
  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Tournament report"
        className="modal tournament-report"
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <header className="crosstable-head">
          <div>
            <strong>
              {title}
              {table?.site ? ` · ${table.site}` : ""}
            </strong>
            {table && (
              <div className="hint">
                {table.players.length} players · {table.games} games ·{" "}
                {table.rounds} rounds
                {table.averageElo ? ` · Ø ${table.averageElo}` : ""}
                {table.category ? ` · category ${table.category}` : ""}
                {table.truncated ? " · stopped at the limit" : ""}
              </div>
            )}
          </div>
        </header>
        {loading && <div className="empty-state">Reading the games…</div>}
        {error && <div className="empty-state">{error}</div>}
        {report && table && !loading && (
          <div className="report-body">
            {s && (
              <p className="report-stats">
                White wins {s.whiteWins} · draws {s.draws} · Black wins{" "}
                {s.blackWins}
                {s.forfeits ? ` · forfeits ${s.forfeits}` : ""}
                {s.whiteScore != null
                  ? ` · White scores ${s.whiteScore.toFixed(1)}%`
                  : ""}
                {s.averageMoves != null
                  ? ` · ${s.averageMoves} moves on average`
                  : ""}
              </p>
            )}
            <section aria-label="Standings">
              <h3>Standings</h3>
              <table className="stat-table report-standings">
                <thead>
                  <tr>
                    <th className="num">#</th>
                    <th>Player</th>
                    <th className="num">Elo</th>
                    <th className="num">Pts.</th>
                    <th className="num">N</th>
                    <th className="num">SB</th>
                    <th className="num">Perf.</th>
                  </tr>
                </thead>
                <tbody>
                  {table.players.map((p, i) => (
                    <tr key={p.id}>
                      <td className="num">{i + 1}</td>
                      <td>{p.name ?? "?"}</td>
                      <td className="num">{p.elo ?? ""}</td>
                      <td className="num strong">{points(p.points)}</td>
                      <td className="num">{p.games}</td>
                      <td className="num">{p.sonnebornBerger.toFixed(2)}</td>
                      <td className="num">{p.performance ?? ""}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
            <div className="report-options" role="group" aria-label="Options">
              <label>
                Rounds from{" "}
                <input
                  type="number"
                  min={1}
                  value={from}
                  placeholder="1"
                  onChange={(e) => setFrom(e.target.value)}
                  aria-label="First round"
                />
              </label>
              <label>
                to{" "}
                <input
                  type="number"
                  min={1}
                  value={to}
                  placeholder={String(table.rounds || 1)}
                  onChange={(e) => setTo(e.target.value)}
                  aria-label="Last round"
                />
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={twoColumns}
                  onChange={(e) => setTwoColumns(e.target.checked)}
                />{" "}
                Two columns
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={standings}
                  onChange={(e) => setStandings(e.target.checked)}
                />{" "}
                Standings after each round
              </label>
            </div>
            <div className={twoColumns ? "report-rounds two" : "report-rounds"}>
              {report.rounds.filter(shown).map((r) => {
                const label = r.round ? `Round ${r.round}` : "Without a round";
                return (
                  <section key={r.round} aria-label={label}>
                    <h3>
                      {label}
                      {r.date ? ` – ${dateText(r.date)}` : ""}{" "}
                      <button
                        type="button"
                        className="link-button"
                        onClick={() =>
                          onListGames(
                            r.games.map((g) => String(g.ordinal)),
                            `${title}: ${label.toLowerCase()}`,
                          )
                        }
                      >
                        Games
                      </button>
                    </h3>
                    <table className="stat-table">
                      <tbody>
                        {r.games.map((g) => (
                          <tr
                            key={g.ordinal}
                            title="Double-click to load this game"
                            onDoubleClick={() => onLoadGame(String(g.ordinal))}
                          >
                            <td>{name(g.white)}</td>
                            <td>–</td>
                            <td>{name(g.black)}</td>
                            <td>{g.result}</td>
                            <td className="num">{g.moves}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                    {standings && r.round > 0 && r.standings.length > 0 && (
                      <p className="hint">
                        After round {r.round}:{" "}
                        {r.standings
                          .map((p) => `${name(p.row)} ${points(p.points)}`)
                          .join(", ")}
                      </p>
                    )}
                  </section>
                );
              })}
            </div>
          </div>
        )}
        <div className="modal-actions">
          <span className="hint">Double-click a game to load it.</span>
          <div className="actions">
            <button type="button" disabled={!table} onClick={onCrosstable}>
              Cross table
            </button>
            <button type="button" autoFocus onClick={onClose}>
              Close
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
