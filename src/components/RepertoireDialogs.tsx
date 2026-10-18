import { useEffect, useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type {
  FrequentPlayer,
  GeneratedRepertoire,
  HitGame,
  PositionHits,
  RepertoireProgress,
  RepertoireReport,
} from "../api/types";

const message = (e: unknown) =>
  (e as { message?: string })?.message ?? String(e);

const stages: Record<RepertoireProgress["stage"], string> = {
  games: "Finding the games",
  lines: "Reading the moves",
  writing: "Writing the databases",
  repertoire: "Reading the repertoire",
  scan: "Scanning the games",
};

function progressText(p: RepertoireProgress) {
  const pct = Number((BigInt(p.done) * 100n) / BigInt(p.total || "1"));
  return `${stages[p.stage]}, ${Math.min(pct, 100)}%`;
}

/**
 * Generate Repertoire (the reference app's `GenerateRepertoireDBsCmd`):
 * White and Black repertoire databases from the chosen players' games in
 * this database, one game per ECO code with the repertoire moves marked.
 */
export function GenerateRepertoireDialog({
  api,
  databaseId,
  onGenerated,
  onOpen,
  onClose,
}: {
  api: WorkbenchApi;
  databaseId: string;
  /** The databases written, for the Repertoire Report. */
  onGenerated?: (result: GeneratedRepertoire) => void;
  /** Opens a written repertoire database. */
  onOpen: (path: string) => void;
  onClose: () => void;
}) {
  const [players, setPlayers] = useState<FrequentPlayer[]>();
  const [chosen, setChosen] = useState<Set<number>>(new Set());
  const [folder, setFolder] = useState("");
  const [progress, setProgress] = useState<string>();
  const [error, setError] = useState<string>();
  const [result, setResult] = useState<GeneratedRepertoire>();
  const running = useRef(false);

  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        let list: FrequentPlayer[];
        try {
          list = await api.frequentPlayers(databaseId);
        } catch (e) {
          if ((e as { code?: string })?.code !== "noNameList") throw e;
          setProgress("Listing the players");
          const built = await api.nameList(databaseId, "players", (p) => {
            const pct = Number(
              (BigInt(p.done) * 100n) / BigInt(p.total || "1"),
            );
            if (live) setProgress(`Listing the players, ${pct}%`);
          });
          if (built.rows == null) throw new Error("Listing stopped");
          list = await api.frequentPlayers(databaseId);
        }
        if (!live) return;
        setProgress(undefined);
        setPlayers(list);
        setChosen(new Set(list.slice(0, 1).map((p) => p.id)));
      } catch (e) {
        if (live) {
          setProgress(undefined);
          setError(message(e));
        }
      }
    })();
    return () => {
      live = false;
    };
  }, [api, databaseId]);

  async function start() {
    if (running.current) return;
    running.current = true;
    setError(undefined);
    setResult(undefined);
    setProgress("Starting");
    try {
      const done = await api.generateRepertoire(
        databaseId,
        [...chosen],
        folder,
        (p) => setProgress(progressText(p)),
      );
      if (done) {
        setResult(done);
        onGenerated?.(done);
      } else setError("Generate Repertoire stopped; nothing was written.");
    } catch (e) {
      setError(message(e));
    } finally {
      running.current = false;
      setProgress(undefined);
    }
  }

  const close = () => {
    if (running.current) void api.cancelRepertoire(databaseId);
    onClose();
  };
  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Generate Repertoire"
        className="modal repertoire"
        onKeyDown={(e) => e.key === "Escape" && close()}
      >
        <header className="crosstable-head">
          <div>
            <strong>Generate Repertoire</strong>
            <div className="hint">
              White and Black repertoire databases (2CBH) from your own games:
              one game per opening code you played at least twice, with your
              moves where the opening table ends marked as repertoire moves.
            </div>
          </div>
        </header>
        <div className="repertoire-body">
          <fieldset disabled={!!progress || !players}>
            <legend>Your names</legend>
            {players?.map((p) => (
              <label key={p.id} className="check-row">
                <input
                  type="checkbox"
                  checked={chosen.has(p.id)}
                  onChange={(e) => {
                    const next = new Set(chosen);
                    if (e.target.checked) next.add(p.id);
                    else next.delete(p.id);
                    setChosen(next);
                  }}
                />
                {p.name ?? "?"}{" "}
                <span className="hint">
                  ({p.games} {p.games === 1 ? "game" : "games"})
                </span>
              </label>
            ))}
            {players?.length === 0 && (
              <p className="hint">The database has no players.</p>
            )}
          </fieldset>
          <label className="folder-row">
            Folder
            <input
              type="text"
              value={folder}
              readOnly
              placeholder="Choose a folder"
              aria-label="Folder"
            />
            <button
              type="button"
              disabled={!!progress}
              onClick={() =>
                void api.chooseRepertoireFolder().then((f) => {
                  if (typeof f === "string") setFolder(f);
                })
              }
            >
              Choose…
            </button>
          </label>
          {progress && (
            <div className="empty-state">
              {progress}{" "}
              <button
                type="button"
                className="link-button"
                onClick={() => void api.cancelRepertoire(databaseId)}
              >
                Stop
              </button>
            </div>
          )}
          {error && <p role="alert">{error}</p>}
          {result && (
            <table className="stat-table" aria-label="Result">
              <thead>
                <tr>
                  <th>Database</th>
                  <th className="num" title="Your games with the colour">
                    Games
                  </th>
                  <th className="num" title="Repertoire games written">
                    Openings
                  </th>
                  <th className="num" title="Games merged into them">
                    Merged
                  </th>
                  <th className="num" title="Openings played once, left out">
                    Rare
                  </th>
                  <th
                    className="num"
                    title="Games the opening table does not classify, set-ups and unreadable games"
                  >
                    Unknown
                  </th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {result.sides.map((s) => (
                  <tr key={String(s.white)}>
                    <td>
                      {s.white ? "Repertoire White" : "Repertoire Black"}
                      {s.truncated && (
                        <span className="hint"> (latest {s.read} read)</span>
                      )}
                    </td>
                    <td className="num">{s.games}</td>
                    <td className="num">{s.repertoireGames}</td>
                    <td className="num">{s.merged}</td>
                    <td className="num">{s.rare}</td>
                    <td className="num">{s.unclassified}</td>
                    <td>
                      {s.path ? (
                        <button type="button" onClick={() => onOpen(s.path!)}>
                          Open
                        </button>
                      ) : (
                        <span className="hint">not written</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
        <div className="modal-actions">
          <span className="hint">
            {result
              ? `From the games of ${result.players.join(", ")}.`
              : "Tick the names you play under."}
          </span>
          <div className="actions">
            <button
              type="button"
              disabled={!!progress || chosen.size === 0 || !folder || !players}
              onClick={() => void start()}
            >
              Generate
            </button>
            <button type="button" onClick={close}>
              Close
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

const hitText = (g: HitGame) => {
  const side = (name?: string | null, elo?: number | null) =>
    `${name ?? "?"}${elo ? ` ${elo}` : ""}`;
  return `${side(g.white, g.whiteElo)} – ${side(g.black, g.blackElo)}`;
};

/**
 * Repertoire Report White/Black (the reference app's `GENERATEREPERTOIRE`):
 * the games of this database that reach the positions a repertoire
 * database marks, by topic, the strongest first.
 */
export function RepertoireReportDialog({
  api,
  databaseId,
  white,
  initialPath,
  onListGames,
  onLoadGame,
  onClose,
}: {
  api: WorkbenchApi;
  databaseId: string;
  white: boolean;
  /** The repertoire database last generated or chosen, if any. */
  initialPath?: string;
  onListGames: (p: PositionHits) => void;
  onLoadGame: (ordinal: string) => void;
  onClose: () => void;
}) {
  const colour = white ? "White" : "Black";
  const [path, setPath] = useState(initialPath ?? "");
  const [progress, setProgress] = useState<string>();
  const [error, setError] = useState<string>();
  const [report, setReport] = useState<RepertoireReport>();
  const running = useRef(false);

  async function run(repertoire: string) {
    if (running.current || !repertoire) return;
    running.current = true;
    setError(undefined);
    setReport(undefined);
    setProgress("Starting");
    try {
      setReport(
        await api.repertoireReport(databaseId, repertoire, (p) =>
          setProgress(progressText(p)),
        ),
      );
    } catch (e) {
      setError(message(e));
    } finally {
      running.current = false;
      setProgress(undefined);
    }
  }
  const close = () => {
    if (running.current) void api.cancelRepertoire(databaseId);
    onClose();
  };
  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label={`Repertoire Report ${colour}`}
        className="modal repertoire report"
        onKeyDown={(e) => e.key === "Escape" && close()}
      >
        <header className="crosstable-head">
          <div>
            <strong>Repertoire Report {colour}</strong>
            {report && (
              <div className="hint">
                {report.repertoire}: {report.repertoireGames} repertoire games,{" "}
                {report.positions} marked positions
                {report.repertoireTruncated ? " (the first 5,000 games)" : ""}
                {" · "}
                {report.matched} of {report.scanned} games reach one
                {report.scanTruncated ? " (the latest games scanned)" : ""}
                {report.cancelled ? " · stopped" : ""}
              </div>
            )}
          </div>
        </header>
        <div className="repertoire-body">
          <label className="folder-row">
            Repertoire
            <input
              type="text"
              value={path}
              readOnly
              placeholder={`Choose the ${colour} repertoire database`}
              aria-label="Repertoire database"
            />
            <button
              type="button"
              disabled={!!progress}
              onClick={() =>
                void api.chooseRepertoireDatabase(white).then((f) => {
                  if (typeof f === "string") setPath(f);
                })
              }
            >
              Choose…
            </button>
          </label>
          {progress && (
            <div className="empty-state">
              {progress}{" "}
              <button
                type="button"
                className="link-button"
                onClick={() => void api.cancelRepertoire(databaseId)}
              >
                Stop
              </button>
            </div>
          )}
          {error && <p role="alert">{error}</p>}
          {report && report.topics.length === 0 && !progress && (
            <p className="hint">
              {report.positions === 0
                ? "The repertoire marks no opening positions. Mark repertoire moves as critical opening positions."
                : "No game of this database reaches a repertoire position."}
            </p>
          )}
          {report && report.topics.length > 0 && (
            <div className="stat-scroll repertoire-topics">
              {report.topics.map((t, ti) => (
                <section key={ti} aria-label={t.name}>
                  <h3>{[t.eco, t.name].filter(Boolean).join(" ")}</h3>
                  {t.positions.map((p) => (
                    <div key={p.fen} className="repertoire-position">
                      <div
                        className="prepare-line"
                        title="Double-click to list these games"
                        onDoubleClick={() => onListGames(p)}
                      >
                        {p.text}{" "}
                        <span className="hint">
                          ({p.games} {p.games === 1 ? "game" : "games"})
                        </span>
                      </div>
                      <ul>
                        {p.top.map((g) => (
                          <li
                            key={g.ordinal}
                            title="Double-click to load the game"
                            onDoubleClick={() => onLoadGame(g.ordinal)}
                          >
                            {hitText(g)} {g.result ?? ""}{" "}
                            {g.year ? `(${g.year})` : ""}
                          </li>
                        ))}
                      </ul>
                    </div>
                  ))}
                </section>
              ))}
              {report.positionsTruncated && (
                <p className="hint">
                  Only the first 1,000 positions are shown.
                </p>
              )}
            </div>
          )}
        </div>
        <div className="modal-actions">
          <span className="hint">
            Double-click a line to list its best games, or a game to load it.
          </span>
          <div className="actions">
            <button
              type="button"
              disabled={!!progress || !path}
              onClick={() => void run(path)}
            >
              Report
            </button>
            <button type="button" onClick={close}>
              Close
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
