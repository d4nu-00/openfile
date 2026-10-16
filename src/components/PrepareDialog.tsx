import type {
  Preparation,
  PreparedGame,
  RepertoireRow,
  StatScore,
} from "../api/types";

const pct = (s: StatScore) => (s.percent == null ? "" : `${s.percent}%`);
const score = (v?: number) => (v == null ? "" : `${v.toFixed(0)}%`);

/**
 * Prepare against White or Black (ChessBase's `PREPARE_WHITE`,
 * `PREPARE_BLACK`): the player's games with one colour. ChessBase opens a
 * game window on them; the workbench shows what they hold, can show their
 * tree in the Book pane, list them and load the last one.
 */
export function PrepareDialog({
  name,
  white,
  data,
  progress,
  error,
  onColor,
  onCancel,
  onShowInBook,
  onListGames,
  onLoadGame,
  onClose,
}: {
  name?: string;
  white: boolean;
  data?: Preparation;
  progress?: string;
  error?: string;
  onColor: (white: boolean) => void;
  onCancel: () => void;
  onShowInBook: () => void;
  /** Lists all the games, or those of a repertoire line. */
  onListGames: (line?: RepertoireRow) => void;
  onLoadGame: (ordinal: string) => void;
  onClose: () => void;
}) {
  const colour = white ? "White" : "Black";
  const who = data?.name ?? name ?? "?";
  const latest = (g: PreparedGame) =>
    [g.tournament, g.year].filter(Boolean).join(" ");
  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label={`Prepare against ${colour}`}
        className="modal prepare"
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <header className="crosstable-head">
          <div>
            <strong>
              Prepare against {colour}: {who}
            </strong>
            {data && (
              <div className="hint">
                {data.games} {data.games === 1 ? "game" : "games"} with {colour}
                {data.firstYear
                  ? ` from ${data.firstYear}${data.lastYear !== data.firstYear ? `–${data.lastYear}` : ""}`
                  : ""}
                {data.deleted ? ` · ${data.deleted} deleted, left out` : ""}
                {data.read < data.games
                  ? ` · the moves of the latest ${data.read} read`
                  : ""}
                {data.unreadable ? ` · ${data.unreadable} unreadable` : ""}
                {data.truncated ? " · stopped at the limit" : ""}
              </div>
            )}
          </div>
          <div className="segmented" role="group" aria-label="Colour">
            {[true, false].map((w) => (
              <button
                key={String(w)}
                type="button"
                aria-pressed={white === w}
                disabled={!!progress}
                onClick={() => onColor(w)}
              >
                {w ? "As White" : "As Black"}
              </button>
            ))}
          </div>
        </header>
        {progress && (
          <div className="empty-state">
            {progress}{" "}
            <button type="button" className="link-button" onClick={onCancel}>
              Stop
            </button>
          </div>
        )}
        {error && <div className="empty-state">{error}</div>}
        {data && !progress && (
          <div className="prepare-body">
            <table className="stat-table prepare-score" aria-label="Score">
              <thead>
                <tr>
                  <th className="num">N</th>
                  <th className="num">+</th>
                  <th className="num">=</th>
                  <th className="num">−</th>
                  <th className="num">Score</th>
                  <th className="num" title="Average Elo of the opponents">
                    Elo-Av
                  </th>
                  <th className="num">Perf.</th>
                  <th
                    className="num"
                    title="Average rating of the player's last ten rated games with this colour"
                  >
                    Recent Elo
                  </th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td className="num">{data.score.games}</td>
                  <td className="num">{data.score.wins}</td>
                  <td className="num">{data.score.draws}</td>
                  <td className="num">{data.score.losses}</td>
                  <td className="num strong">{pct(data.score)}</td>
                  <td className="num">{data.score.averageElo ?? ""}</td>
                  <td className="num">{data.score.performance ?? ""}</td>
                  <td className="num">{data.recentRating ?? ""}</td>
                </tr>
              </tbody>
            </table>
            <div className="prepare-lists">
              <section aria-label="Repertoire">
                <div className="stat-scroll">
                  <table className="stat-table">
                    <thead>
                      <tr>
                        <th>Line</th>
                        <th>Opening</th>
                        <th className="num">N</th>
                        <th className="num" title="The player's score">
                          Score
                        </th>
                        <th className="num">Last</th>
                      </tr>
                    </thead>
                    <tbody>
                      {data.repertoire.map((l) => (
                        <tr
                          key={l.moves.join(" ")}
                          title="Double-click to list these games"
                          onDoubleClick={() => onListGames(l)}
                        >
                          <td className="prepare-line">{l.text}</td>
                          <td title={l.name}>
                            {[l.eco, l.name].filter(Boolean).join(" ")}
                          </td>
                          <td className="num">{l.games}</td>
                          <td className="num">{score(l.score)}</td>
                          <td className="num">{l.lastYear ?? ""}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  {data.repertoire.length === 0 && (
                    <p className="hint">No line is played often enough.</p>
                  )}
                </div>
              </section>
              <section aria-label="Openings">
                <div className="stat-scroll">
                  <table className="stat-table">
                    <thead>
                      <tr>
                        <th>ECO</th>
                        <th className="num">N</th>
                        <th className="num">Score</th>
                      </tr>
                    </thead>
                    <tbody>
                      {data.openings.map((o) => (
                        <tr key={o.eco ?? "none"}>
                          <td>{o.eco ?? "none"}</td>
                          <td className="num">{o.score.games}</td>
                          <td className="num">{pct(o.score)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </section>
              <section aria-label="Latest games">
                <div className="stat-scroll">
                  <table className="stat-table">
                    <thead>
                      <tr>
                        <th>Opponent</th>
                        <th className="num">Elo</th>
                        <th>Result</th>
                        <th>ECO</th>
                        <th>Event</th>
                      </tr>
                    </thead>
                    <tbody>
                      {data.latest.map((g) => (
                        <tr
                          key={g.ordinal}
                          title="Double-click to load this game"
                          onDoubleClick={() => onLoadGame(g.ordinal)}
                        >
                          <td>{g.opponent ?? "?"}</td>
                          <td className="num">{g.opponentElo ?? ""}</td>
                          <td>{g.result ?? ""}</td>
                          <td>{g.eco ?? ""}</td>
                          <td>{latest(g)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </section>
            </div>
          </div>
        )}
        <div className="modal-actions">
          <span className="hint">
            Double-click a line to list its games, or a game to load it.
          </span>
          <div className="actions">
            <button
              type="button"
              disabled={!data || !!progress}
              onClick={onShowInBook}
            >
              Show in Book
            </button>
            <button
              type="button"
              disabled={!data || !!progress || data.games === 0}
              onClick={() => onListGames()}
            >
              List games
            </button>
            <button
              type="button"
              disabled={!data?.lastGame || !!progress}
              onClick={() => data?.lastGame && onLoadGame(data.lastGame)}
            >
              Load last game
            </button>
            <button type="button" onClick={onClose}>
              Close
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
