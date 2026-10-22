import type { NoveltyReport } from "../api/types";

/** Find Novelty's answer: the first new move and the games before it. */
export function NoveltyDialog({
  report,
  running,
  error,
  databaseName,
  moveName,
  onSelectNovelty,
  onStop,
  onClose,
}: {
  report?: NoveltyReport;
  running: boolean;
  error?: string;
  databaseName: string;
  /** "14...Nd7" for a ply of the main line. */
  moveName: (ply: number) => string;
  onSelectNovelty: () => void;
  onStop: () => void;
  onClose: () => void;
}) {
  const r = report;
  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Find Novelty"
        className="modal novelty"
        onKeyDown={(e) => e.key === "Escape" && !running && onClose()}
      >
        <h2>Find Novelty</h2>
        {running && <p role="status">Searching {databaseName}…</p>}
        {r?.cancelled && <p role="status">Stopped.</p>}
        {error && <p role="alert">{error}</p>}
        {r?.notSearched && (
          <p role="alert">
            {databaseName} has no opening tree or search booster, and is too
            large to read for every move. Build one, or search the position with
            Find Position.
          </p>
        )}
        {r && !r.cancelled && !r.notSearched && (
          <>
            <p>
              {r.novelty ? (
                <>
                  The novelty is <strong>{moveName(r.novelty.ply)}</strong>
                  {r.knownPly > 0
                    ? `: the position before it occurs in ${r.total} other game${r.total === 1 ? "" : "s"} of ${databaseName}.`
                    : `: no other game of ${databaseName} starts from this position.`}
                </>
              ) : (
                `Every move of the main line occurs in other games of ${databaseName} (${r.total} reach its end).`
              )}
            </p>
            {r.games.length > 0 && (
              <div className="table-scroll">
                <table aria-label="Games before the novelty">
                  <thead>
                    <tr>
                      <th>#</th>
                      <th>White</th>
                      <th>Black</th>
                      <th>Result</th>
                      <th>Date</th>
                      <th>Event</th>
                    </tr>
                  </thead>
                  <tbody>
                    {r.games.map((g) => (
                      <tr key={g.ordinal}>
                        <td className="num">{g.ordinal}</td>
                        <td>{g.white}</td>
                        <td>{g.black}</td>
                        <td>{g.result}</td>
                        <td>{g.date}</td>
                        <td>{g.event}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                {r.total > r.games.length && (
                  <p className="hint">
                    The first {r.games.length} of {r.total} games.
                  </p>
                )}
              </div>
            )}
          </>
        )}
        <div className="modal-actions">
          {r?.novelty && (
            <button type="button" onClick={onSelectNovelty}>
              Go to the novelty
            </button>
          )}
          {running ? (
            <button type="button" onClick={onStop}>
              Stop
            </button>
          ) : (
            <button type="button" onClick={onClose}>
              Close
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
