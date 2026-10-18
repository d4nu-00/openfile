import type { CrossGame, CrossLayout, Crosstable } from "../api/types";

const points = (p: number) =>
  Number.isInteger(p) ? String(p) : `${Math.floor(p) || ""}½`;
const tiebreak = (v: number) =>
  Number.isInteger(v) ? String(v) : v.toFixed(2).replace(/0$/, "");

/**
 * A tournament crosstable (the reference app's Cross Table, T). A round robin has a
 * column per opponent; a Swiss tournament a column per round, naming the
 * opponent's place and colour. Each result opens its game.
 */
export function CrosstableDialog({
  table,
  loading,
  error,
  onLayout,
  onOpenGame,
  onReport,
  onClose,
}: {
  table?: Crosstable;
  loading?: boolean;
  error?: string;
  onLayout: (layout: CrossLayout) => void;
  onOpenGame: (ordinal: number) => void;
  /** Opens the tournament report. */
  onReport?: () => void;
  onClose: () => void;
}) {
  const swiss = table?.layout === "swiss";
  const columns = table
    ? swiss
      ? Math.max(1, table.rounds)
      : table.players.length
    : 0;
  const cell = (games: CrossGame[], row: number, column: number) => {
    if (!swiss && row === column)
      return <td key={column} className="cross-self" aria-hidden />;
    return (
      <td key={column} className="cross-cell">
        {games.map((g) => (
          <button
            key={g.ordinal}
            className="cross-game"
            title={`Round ${g.round || "?"} · ${g.white ? "White" : "Black"} · game ${g.ordinal}`}
            onClick={() => onOpenGame(g.ordinal)}
          >
            {swiss
              ? `${g.opponent + 1}${g.white ? "w" : "b"}${g.mark}`
              : g.mark}
          </button>
        ))}
      </td>
    );
  };
  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Cross table"
        className="modal crosstable"
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <header className="crosstable-head">
          <div>
            <strong>{table?.event ?? "Cross table"}</strong>
            {table?.site && <span className="hint"> · {table.site}</span>}
            {table && (
              <div className="hint">
                {table.players.length} players · {table.games} games
                {table.rounds ? ` · ${table.rounds} rounds` : ""}
                {table.averageElo ? ` · Average Elo ${table.averageElo}` : ""}
                {table.category ? ` · Category ${table.category}` : ""}
                {table.unscored
                  ? ` · ${table.unscored} without a result, not counted`
                  : ""}
                {table.truncated ? " · stopped at the limit" : ""}
              </div>
            )}
          </div>
          <div className="segmented" role="group" aria-label="Layout">
            <button
              aria-pressed={table?.layout === "roundRobin"}
              onClick={() => onLayout("roundRobin")}
            >
              Round robin
            </button>
            <button aria-pressed={swiss} onClick={() => onLayout("swiss")}>
              Swiss
            </button>
          </div>
        </header>
        <div className="crosstable-body">
          {loading && <div className="empty-state">Reading the games…</div>}
          {error && <div className="empty-state">{error}</div>}
          {table && !loading && (
            <table>
              <thead>
                <tr>
                  <th className="num">Rk.</th>
                  <th>Name</th>
                  <th className="num">Elo</th>
                  {Array.from({ length: columns }, (_, i) => (
                    <th key={i} className="center">
                      {i + 1}
                    </th>
                  ))}
                  <th className="num">Pts.</th>
                  <th className="num" title="Games">
                    G
                  </th>
                  <th className="num" title="Number of wins">
                    #W
                  </th>
                  <th
                    className="num"
                    title={swiss ? "Buchholz" : "Sonneborn-Berger"}
                  >
                    {swiss ? "BH" : "SB"}
                  </th>
                  <th
                    className="num"
                    title={swiss ? "Sonneborn-Berger" : "Buchholz"}
                  >
                    {swiss ? "SB" : "BH"}
                  </th>
                  <th className="num" title="Performance">
                    Perf.
                  </th>
                </tr>
              </thead>
              <tbody>
                {table.players.map((p, row) => (
                  <tr key={p.id}>
                    <td className="num">{row + 1}</td>
                    <td>{p.name ?? "?"}</td>
                    <td className="num">{p.elo || ""}</td>
                    {Array.from({ length: columns }, (_, c) =>
                      cell(p.cells[c] ?? [], row, c),
                    )}
                    <td className="num strong">{points(p.points)}</td>
                    <td className="num">{p.games}</td>
                    <td className="num">{p.wins}</td>
                    <td className="num">
                      {tiebreak(swiss ? p.buchholz : p.sonnebornBerger)}
                    </td>
                    <td className="num">
                      {tiebreak(swiss ? p.sonnebornBerger : p.buchholz)}
                    </td>
                    <td className="num">{p.performance ?? ""}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
        <footer className="modal-actions">
          {onReport && (
            <button type="button" disabled={!table} onClick={onReport}>
              Report
            </button>
          )}
          <button type="button" onClick={onClose} autoFocus>
            Close
          </button>
        </footer>
      </div>
    </div>
  );
}
