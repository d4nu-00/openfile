import type { TreeMove } from "../api/types";
import type { BookState } from "../state/positionPanes";

const percent = (v?: number | null) =>
  v === undefined || v === null ? "" : `${v.toFixed(1)}%`;
const number = (v?: number | null) =>
  v === undefined || v === null ? "" : String(v);
const games = new Intl.NumberFormat("en-US");

/** The mover's wins, draws and losses as a bar, from % and =%. */
function ResultBar({ move }: { move: TreeMove }) {
  if (move.score === undefined || move.score === null) return null;
  const draws = move.draws ?? 0;
  const wins = Math.max(0, move.score - draws / 2);
  const losses = Math.max(0, 100 - wins - draws);
  return (
    <span
      className="result-bar"
      title={`Wins ${wins.toFixed(0)}% · draws ${draws.toFixed(0)}% · losses ${losses.toFixed(0)}%`}
    >
      <span className="win" style={{ width: `${wins}%` }} />
      <span className="draw" style={{ width: `${draws}%` }} />
      <span className="loss" style={{ width: `${losses}%` }} />
    </span>
  );
}

const sources = {
  tree: "Opening tree",
  line: "Followed from the last stored position",
  notInTree: "",
  tooMany: "",
  noTree: "",
  filtered: "Search results",
};

/**
 * The opening tree of the open database for the current position, with
 * The reference app's book columns (N, %, =%, Av, Perf) and the last year played.
 */
export function BookPane({
  book,
  hasDatabase,
  hasLine,
  onPlay,
  onCreate,
  building,
  prepared,
  onLeavePrepared,
  searchAvailable = false,
  searchTruncated = false,
  scope = "database",
  onScopeChange,
}: {
  book: BookState;
  hasDatabase: boolean;
  hasLine: boolean;
  onPlay: (uci: string) => void;
  onCreate?: () => void;
  /** Progress of a running build, if any. */
  building?: string;
  /** "Prepare against White: name" while the Book shows a preparation. */
  prepared?: string;
  /** Returns to the database's opening tree. */
  onLeavePrepared?: () => void;
  searchAvailable?: boolean;
  searchTruncated?: boolean;
  scope?: "database" | "search";
  onScopeChange?: (scope: "database" | "search") => void;
}) {
  const tree = book.tree;
  let message: string | undefined;
  if (!hasDatabase) message = "Open a database to see its opening tree.";
  else if (!hasLine) message = "Select a game to see the moves played.";
  else if (book.error) message = book.error;
  else if (prepared && tree?.status === "noTree")
    message = "The preparation is no longer current.";
  else if (prepared && tree?.status === "notInTree")
    message = "The player's games do not reach this position.";
  else if (tree?.status === "noTree")
    message = building ?? "This database has no opening tree yet.";
  else if (tree?.status === "notInTree")
    message = "This position is not in the opening tree.";
  else if (tree?.status === "tooMany")
    message =
      "Too many games to follow past the stored positions; use Find Position.";
  else if (tree && tree.games === 0)
    message = prepared
      ? "No game in this preparation reaches this position."
      : scope === "search"
        ? "No game in this search result reaches this position."
        : "No game of this database reaches this position.";
  return (
    <div className="book-pane" aria-label="Opening book">
      <div className="pane-header">
        <strong>Book</strong>
        {searchAvailable && onScopeChange && (
          <span className="book-scope" role="group" aria-label="Book source">
            <button
              type="button"
              className={scope === "database" ? "active" : ""}
              aria-pressed={scope === "database"}
              onClick={() => onScopeChange("database")}
            >
              Database tree
            </button>
            <button
              type="button"
              className={scope === "search" ? "active" : ""}
              aria-pressed={scope === "search"}
              onClick={() => onScopeChange("search")}
            >
              Search results
            </button>
          </span>
        )}
        {prepared && (
          <span className="book-prepared">
            {prepared}{" "}
            {onLeavePrepared && (
              <button
                type="button"
                className="link-button"
                onClick={onLeavePrepared}
              >
                Database tree
              </button>
            )}
          </span>
        )}
        {tree && tree.games > 0 && (
          <span className="hint">
            {games.format(tree.games)} games
            {tree.ended > 0 ? ` · ${games.format(tree.ended)} end here` : ""}
            {!prepared && sources[tree.status]
              ? ` · ${sources[tree.status]}`
              : ""}
          </span>
        )}
        {book.loading && <span className="hint">Updating…</span>}
        {tree?.status === "filtered" && (
          <span className="hint">
            {games.format(tree.sampledHits ?? 0)} of{" "}
            {games.format(tree.totalHits ?? 0)} search hits scanned
          </span>
        )}
        {tree?.status === "filtered" && searchTruncated && (
          <span className="hint">Search stopped at its limit</span>
        )}
      </div>
      {message ? (
        <div className="empty-state">
          <p>{message}</p>
          {!prepared && tree?.status === "noTree" && onCreate && !building && (
            <button onClick={onCreate}>Create Opening Tree</button>
          )}
        </div>
      ) : (
        <div className="game-list book-list">
          <table>
            <thead>
              <tr>
                <th>Move</th>
                <th className="num" title="Games">
                  N
                </th>
                <th className="num" title="Share of this position's games">
                  Share
                </th>
                <th title="Score of the side to move">%</th>
                <th className="num" title="Draws">
                  =%
                </th>
                <th className="num" title="Average rating of the mover">
                  Av
                </th>
                <th className="num" title="The mover's performance">
                  Perf
                </th>
                <th className="num">Last played</th>
              </tr>
            </thead>
            <tbody>
              {tree?.moves.map((m) => (
                <tr key={m.uci} onClick={() => onPlay(m.uci)}>
                  <td>
                    <button
                      className="book-move"
                      onClick={(ev) => {
                        ev.stopPropagation();
                        onPlay(m.uci);
                      }}
                      aria-label={`Play ${m.san}`}
                    >
                      {m.san}
                    </button>
                  </td>
                  <td className="num">{games.format(m.games)}</td>
                  <td className="num">{percent(m.share)}</td>
                  <td className="score">
                    <ResultBar move={m} />
                    <span>{percent(m.score)}</span>
                  </td>
                  <td className="num">
                    {m.draws === undefined || m.draws === null
                      ? ""
                      : `${m.draws.toFixed(0)}%`}
                  </td>
                  <td className="num">{number(m.averageElo)}</td>
                  <td className="num">{number(m.performance)}</td>
                  <td
                    className="num"
                    title={
                      m.firstYear ? `First played ${m.firstYear}` : undefined
                    }
                  >
                    {number(m.lastYear)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
