import { useEffect, useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type {
  StyleEvidence,
  StyleMetric,
  StyleParams,
  StyleRate,
  StyleReport,
} from "../api/types";

type Player = { id: number; name?: string | null };
const activityIds = new Set([
  "checks",
  "captures",
  "earlyQueen",
  "kingsideCastle",
  "queensideCastle",
  "oppositeCastle",
]);
const percent = (rate: StyleRate) =>
  rate.value == null ? "—" : `${rate.value.toFixed(1)}%`;
const date = (value?: string | null) =>
  value?.replace(/(?:\.\?\?)+$/, "") ?? "unknown date";

function Metric({
  metric: m,
  onEvidence,
}: {
  metric: StyleMetric;
  onEvidence: (e: StyleEvidence) => void;
}) {
  return (
    <section className="style-metric" aria-label={m.label}>
      <div className="style-metric-heading">
        <h3>{m.label}</h3>
        <strong>{percent(m)}</strong>
      </div>
      <p className="style-definition">{m.description}</p>
      <div className="style-counts">
        <span>
          {m.numerator} / {m.denominator}
        </span>
        <span>
          {m.denominator === 0
            ? "Insufficient evidence"
            : m.denominator < m.minimum
              ? `Small sample · below ${m.minimum}`
              : "Observed sample"}
        </span>
      </div>
      <div className="style-colours">
        <span>
          White <b>{percent(m.white)}</b>{" "}
          <small>
            ({m.white.numerator}/{m.white.denominator})
          </small>
          {m.white.denominator > 0 && m.white.denominator < m.minimum
            ? " · small"
            : ""}
        </span>
        <span>
          Black <b>{percent(m.black)}</b>{" "}
          <small>
            ({m.black.numerator}/{m.black.denominator})
          </small>
          {m.black.denominator > 0 && m.black.denominator < m.minimum
            ? " · small"
            : ""}
        </span>
      </div>
      {m.evidence.length > 0 && (
        <details>
          <summary>Supporting positions ({m.evidence.length} shown)</summary>
          <div className="style-evidence">
            {m.evidence.map((e) => (
              <button
                type="button"
                key={`${e.ordinal}-${e.node}`}
                onClick={() => onEvidence(e)}
              >
                Entry {e.ordinal} · ply {e.ply} · {e.san}
              </button>
            ))}
          </div>
        </details>
      )}
    </section>
  );
}

/** Computed local report. Every asynchronous callback is bound to one generation. */
export function PlayingStyleDialog({
  api,
  databaseId,
  databaseName,
  player,
  onLoadGame,
  onClose,
}: {
  api: WorkbenchApi;
  databaseId: string;
  databaseName: string;
  player: Player;
  onLoadGame: (ordinal: string, node: number, jobId: string) => void;
  onClose: () => void;
}) {
  const [players, setPlayers] = useState<Player[]>([player]);
  const [colour, setColour] = useState("both");
  const [yearFrom, setYearFrom] = useState("");
  const [yearTo, setYearTo] = useState("");
  const [minElo, setMinElo] = useState("");
  const [limit, setLimit] = useState("500");
  const [aliasQuery, setAliasQuery] = useState("");
  const [aliases, setAliases] = useState<Player[]>([]);
  const [aliasBusy, setAliasBusy] = useState(false);
  const aliasGeneration = useRef(0);
  const [report, setReport] = useState<StyleReport>();
  const [running, setRunning] = useState<string>();
  const [error, setError] = useState<string>();
  const [section, setSection] = useState("activity");
  const [editCriteria, setEditCriteria] = useState(true);
  const active = useRef<string | undefined>(undefined);
  const reportGeneration = useRef(0);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      reportGeneration.current++;
      aliasGeneration.current++;
      if (active.current)
        void api.cancelPlayingStyle(databaseId, active.current);
      active.current = undefined;
    };
  }, [api, databaseId]);
  function stop() {
    reportGeneration.current++;
    const id = active.current;
    active.current = undefined;
    if (id) void api.cancelPlayingStyle(databaseId, id);
    setRunning(undefined);
    setError("Report stopped. No partial results were published.");
  }
  async function generate() {
    reportGeneration.current++;
    if (active.current) void api.cancelPlayingStyle(databaseId, active.current);
    const id = crypto.randomUUID();
    active.current = id;
    const live = () => mounted.current && active.current === id;
    const params: StyleParams = {
      playerIds: players.map((p) => p.id),
      sampleLimit: Number(limit),
      white: colour === "both" ? null : colour === "white",
      yearFrom: yearFrom ? Number(yearFrom) : null,
      yearTo: yearTo ? Number(yearTo) : null,
      minOpponentElo: minElo ? Number(minElo) : null,
    };
    setReport(undefined);
    setError(undefined);
    setRunning("Finding eligible games…");
    try {
      const result = await api.playingStyle(databaseId, id, params, (p) => {
        if (live())
          setRunning(
            `${p.stage === "scan" || p.stage === "games" ? "Scanning game headers" : "Replaying sampled games"} · ${p.done} / ${p.total}`,
          );
      });
      if (!live()) return;
      active.current = undefined;
      setRunning(undefined);
      if (result && result.jobId === id) {
        setReport(result);
        setEditCriteria(false);
        setSection("activity");
      } else
        setError(
          "Report stopped or superseded. Generate again for the current source.",
        );
    } catch (e) {
      if (!live()) return;
      active.current = undefined;
      setRunning(undefined);
      setError((e as Error).message ?? String(e));
    }
  }
  async function searchAlias() {
    const generation = ++aliasGeneration.current;
    setAliasBusy(true);
    setError(undefined);
    try {
      await api.nameList(databaseId, "players", () => {});
      const start = await api.findName(databaseId, "players", aliasQuery);
      const page = await api.readNames(
        databaseId,
        "players",
        start,
        8,
        crypto.randomUUID(),
      );
      if (!mounted.current || generation !== aliasGeneration.current) return;
      setAliases(page.rows.filter((p) => !players.some((v) => v.id === p.id)));
    } catch (e) {
      if (mounted.current && generation === aliasGeneration.current)
        setError((e as Error).message ?? String(e));
    } finally {
      if (mounted.current && generation === aliasGeneration.current)
        setAliasBusy(false);
    }
  }
  async function evidence(ordinal: string, node: number) {
    if (!report) return;
    const generation = reportGeneration.current;
    const live = () =>
      mounted.current && reportGeneration.current === generation;
    try {
      await api.validatePlayingStyle(databaseId, report.jobId);
      if (live()) {
        reportGeneration.current++;
        onLoadGame(ordinal, node, report.jobId);
      }
    } catch (e) {
      if (live()) setError((e as Error).message ?? String(e));
    }
  }
  const criteria = report?.criteria;
  const criteriaText =
    criteria &&
    `${criteria.white == null ? "Both colours" : criteria.white ? "White only" : "Black only"} · ${criteria.yearFrom ?? "any year"}–${criteria.yearTo ?? "present"} · opponent Elo ${criteria.minOpponentElo ? `≥ ${criteria.minOpponentElo}` : "any / unknown included"} · maximum ${criteria.sampleLimit}`;
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Style report"
        className="modal playing-style"
        onSubmit={(e) => {
          e.preventDefault();
          if (!running) void generate();
        }}
        onKeyDown={(e) => {
          if (e.key === "Escape") onClose();
        }}
      >
        <header className="style-header">
          <div>
            <span className="style-eyebrow">LOCAL PLAYING STYLE</span>
            <h2>
              {report?.players[0]?.name ?? player.name ?? `Player ${player.id}`}
            </h2>
            <p>
              {report?.database ?? databaseName} ·{" "}
              {report?.databaseEntries ?? ""} database entries
            </p>
          </div>
          <button type="button" onClick={onClose}>
            Close
          </button>
        </header>
        <div className="style-body">
          {(editCriteria || !report) && (
            <fieldset disabled={!!running} className="style-criteria">
              <legend>Games to analyse</legend>
              <p className="style-identities">
                {players.map((p, i) => (
                  <span key={p.id}>
                    {p.name ?? `Player ${p.id}`} <small>#{p.id}</small>
                    {i > 0 && (
                      <button
                        type="button"
                        aria-label={`Remove alias ${p.name}`}
                        onClick={() =>
                          setPlayers(players.filter((v) => v.id !== p.id))
                        }
                      >
                        ×
                      </button>
                    )}
                  </span>
                ))}
              </p>
              <div className="style-filter-grid">
                <label>
                  Colour
                  <select
                    value={colour}
                    onChange={(e) => setColour(e.target.value)}
                  >
                    <option value="both">Both colours</option>
                    <option value="white">White</option>
                    <option value="black">Black</option>
                  </select>
                </label>
                <label>
                  From year
                  <input
                    type="number"
                    min="1"
                    max="4095"
                    value={yearFrom}
                    onChange={(e) => setYearFrom(e.target.value)}
                    placeholder="Any"
                  />
                </label>
                <label>
                  Through year
                  <input
                    aria-label="Through year"
                    type="number"
                    min={yearFrom || "1"}
                    max="4095"
                    value={yearTo}
                    onChange={(e) => setYearTo(e.target.value)}
                    placeholder="Any"
                  />
                </label>
                <label>
                  Minimum opponent Elo
                  <input
                    type="number"
                    min="1"
                    max="4000"
                    value={minElo}
                    onChange={(e) => setMinElo(e.target.value)}
                    placeholder="Any"
                  />
                </label>
                <label>
                  Maximum games
                  <input
                    type="number"
                    required
                    min="1"
                    max="2000"
                    value={limit}
                    onChange={(e) => setLimit(e.target.value)}
                  />
                </label>
              </div>
              <details className="style-aliases">
                <summary>Include another spelling / identity</summary>
                <p>
                  Select identities explicitly. Similar names are never merged
                  automatically.
                </p>
                <div>
                  <input
                    aria-label="Alias name"
                    value={aliasQuery}
                    onChange={(e) => {
                      setAliasQuery(e.target.value);
                      setAliases([]);
                      aliasGeneration.current++;
                      setAliasBusy(false);
                    }}
                    placeholder="Last name, first name"
                  />
                  <button
                    type="button"
                    disabled={
                      !aliasQuery.trim() || aliasBusy || players.length >= 16
                    }
                    onClick={() => void searchAlias()}
                  >
                    {aliasBusy ? "Finding…" : "Find spelling"}
                  </button>
                </div>
                {aliases.map((p) => (
                  <button
                    type="button"
                    key={p.id}
                    disabled={players.length >= 16}
                    onClick={() => {
                      setPlayers([...players, p]);
                      setAliases([]);
                    }}
                  >
                    {p.name} · #{p.id}
                  </button>
                ))}
              </details>
              <p className="style-note">
                Latest dated games first, then entry number; filters apply
                before the sample limit. Deleted games are excluded. Year and
                rating filters exclude unknown values. The active game-list
                search is independent.
              </p>
            </fieldset>
          )}
          {running && (
            <div role="status" className="style-progress">
              {running}
              <progress aria-label="Analysing style" />
            </div>
          )}
          {error && <p role="alert">{error}</p>}
          {report && (
            <>
              <section className="style-context" aria-label="Sample context">
                <strong>
                  {report.analysed} games analysed · {report.whiteGames} White /{" "}
                  {report.blackGames} Black
                </strong>
                <p>
                  {report.selected} sampled from {report.eligible} eligible ·{" "}
                  {date(report.firstDate)} to {date(report.lastDate)}
                </p>
                <p>{criteriaText}</p>
                <p>
                  Identities:{" "}
                  {report.players
                    .map((p) => `${p.name ?? "Unknown"} (#${p.id})`)
                    .join("; ")}
                </p>
                <p>
                  {report.deleted} deleted · {report.filtered} outside filters ·{" "}
                  {report.skipped} skipped · {report.cutGames} cut at 400 plies
                  · {(report.elapsedMs / 1000).toFixed(2)} s
                </p>
                <p className="style-note">
                  {report.unknownDate ?? 0} unknown dates ·{" "}
                  {report.unknownOpponentElo ?? 0} unknown opponent ratings ·{" "}
                  {report.unknownResult ?? 0} unknown results. Results are not
                  required for board measures.
                </p>
                {report.skipped > 0 && (
                  <p className="style-note">
                    Skipped: {report.skippedSetup ?? 0} setups ·{" "}
                    {report.skippedUnreadable ?? 0} unreadable ·{" "}
                    {report.skippedUnsupported ?? 0} unsupported ·{" "}
                    {report.skippedPartial ?? 0} partial.
                  </p>
                )}
              </section>
              <nav className="style-tabs" aria-label="Report sections">
                {[
                  ["activity", "Activity & openings"],
                  ["structure", "Structure & endings"],
                  ["games", "Analysed games"],
                ].map(([key, title]) => (
                  <button
                    type="button"
                    key={key}
                    aria-pressed={section === key}
                    onClick={() => setSection(key)}
                  >
                    {title}
                  </button>
                ))}
              </nav>
              {section !== "games" ? (
                <div className="style-metrics">
                  {report.metrics
                    .filter(
                      (m) => activityIds.has(m.id) === (section === "activity"),
                    )
                    .map((m) => (
                      <Metric
                        key={m.id}
                        metric={m}
                        onEvidence={(e) => void evidence(e.ordinal, e.node)}
                      />
                    ))}
                </div>
              ) : (
                <section aria-label="Analysed games">
                  <p>
                    Every analysed entry, in sample order. Open a game to
                    inspect its main line.
                  </p>
                  <div className="style-game-scroll">
                    <table className="stat-table">
                      <thead>
                        <tr>
                          <th>Entry</th>
                          <th>Colour</th>
                          <th>Opponent</th>
                          <th>Date</th>
                          <th>Result</th>
                          <th>Plies read</th>
                        </tr>
                      </thead>
                      <tbody>
                        {report.games.map((g) => (
                          <tr key={g.ordinal}>
                            <td>
                              <button
                                type="button"
                                onClick={() => void evidence(g.ordinal, 0)}
                              >
                                Open {g.ordinal}
                              </button>
                            </td>
                            <td>{g.white ? "White" : "Black"}</td>
                            <td>{g.opponent ?? "Unknown"}</td>
                            <td>{date(g.date)}</td>
                            <td>{g.result ?? "*"}</td>
                            <td>
                              {g.plies}
                              {g.cut ? " (cut)" : ""}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </section>
              )}
              <p className="style-note">
                Descriptive observations from this sample, without an engine or
                comparison population. Checks do not measure attack quality;
                captures do not prove a preference for exchanges; a material
                deficit does not prove a sacrifice. Small samples are flagged,
                and no observations display as “—”.
              </p>
            </>
          )}
        </div>
        <footer className="modal-actions">
          <span className="style-note">
            Local moves only · no engine time · no result cache
          </span>
          <div>
            {report && (
              <button
                type="button"
                onClick={() => setEditCriteria(!editCriteria)}
              >
                {editCriteria ? "Hide criteria" : "Change criteria"}
              </button>
            )}
            {running ? (
              <button
                type="button"
                onClick={(event) => {
                  // Stopping changes this node back into a submit button.
                  // Suppress WebKit's click default after that synchronous update.
                  event.preventDefault();
                  stop();
                }}
              >
                Stop
              </button>
            ) : (
              <button type="submit">
                {report ? "Generate again" : "Generate report"}
              </button>
            )}
          </div>
        </footer>
      </form>
    </div>
  );
}
