import { useEffect, useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type {
  NativeStyleDimension,
  StyleEvidence,
  StyleParams,
  StyleReport,
} from "../api/types";

type Player = { id: number; name?: string | null };
const date = (value?: string | null) =>
  value?.replace(/(?:\.\?\?)+$/, "") ?? "unknown date";
const number = (value: number | null) =>
  value == null ? "—" : value.toFixed(3);

function Dimension({
  dimension: d,
  onEvidence,
}: {
  dimension: NativeStyleDimension;
  onEvidence: (e: StyleEvidence) => void;
}) {
  return (
    <section className="style-metric" aria-label={d.label}>
      <div className="style-metric-heading">
        <h3>{d.label}</h3>
        <strong>
          {d.normalized == null ? "—" : `${d.normalized.toFixed(1)}%`}
        </strong>
      </div>
      <p className="style-definition">{d.description}</p>
      <div className="style-counts">
        <span>Raw score {number(d.rawScore)}</span>
        <span>
          {d.status === "available"
            ? "Recovered calculation"
            : d.status === "partial"
              ? "Partial evidence"
              : d.status === "insufficient"
                ? "Insufficient evidence"
                : "Not yet reproduced"}
        </span>
      </div>
      {d.unavailableReason && (
        <p className="style-availability">{d.unavailableReason}</p>
      )}
      <p className="style-note">{d.grade ?? "Categorical grade unavailable"}</p>
      <details>
        <summary>Calculation &amp; inputs</summary>
        <p className="style-note">
          Native normalization range: {d.normalization[0]} to{" "}
          {d.normalization[1]}. The normalized score is not a frequency or
          percentile and can exceed 100%.
        </p>
        <p className="style-note">{d.gradeReason}</p>
        {d.inputs.length > 0 && (
          <table className="stat-table style-inputs">
            <thead>
              <tr>
                <th>Input</th>
                <th>Observed</th>
                <th>Numerator / games</th>
                <th>Reference</th>
              </tr>
            </thead>
            <tbody>
              {d.inputs.map((input, i) => (
                <tr key={i}>
                  <td>{input.label}</td>
                  <td>{number(input.value)}</td>
                  <td>
                    {input.numerator == null || input.denominator == null
                      ? "—"
                      : `${input.numerator} / ${input.denominator}`}
                  </td>
                  <td>{number(input.reference)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </details>
      {d.evidence.length > 0 && (
        <details>
          <summary>Supporting positions ({d.evidence.length} shown)</summary>
          <div className="style-evidence">
            {d.evidence.map((e) => (
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
  const [gameTypes, setGameTypes] = useState(4);
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
  const [section, setSection] = useState("dimensions");
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
      gameTypes,
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
        setSection("dimensions");
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
    `${criteria.white == null ? "Both colours" : criteria.white ? "White only" : "Black only"} · ${criteria.yearFrom ?? "any year"}–${criteria.yearTo ?? "present"} · opponent Elo ${criteria.minOpponentElo ? `≥ ${criteria.minOpponentElo}` : "any / unknown included"} · maximum ${criteria.sampleLimit} · ${(
      [
        [4, "Long"],
        [1, "Blitz / bullet"],
        [2, "Rapid"],
        [8, "Simultaneous"],
      ] as const
    )
      .filter(([bit]) => (criteria.gameTypes & bit) !== 0)
      .map(([, label]) => label)
      .join(", ")}`;
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
              <div
                className="style-game-types"
                role="group"
                aria-label="Game types"
              >
                {(
                  [
                    [4, "Long"],
                    [1, "Blitz / bullet"],
                    [2, "Rapid"],
                    [8, "Simultaneous"],
                  ] as const
                ).map(([bit, label]) => (
                  <label key={bit}>
                    <input
                      type="checkbox"
                      checked={(gameTypes & bit) !== 0}
                      onChange={() => setGameTypes(gameTypes ^ bit)}
                    />
                    {label}
                  </label>
                ))}
                <p className="style-note">
                  Native default: Long games. Missing category metadata is
                  included only when all four types are selected. Games with
                  fewer than five stored moves are excluded.
                </p>
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
                {report.analysed > 0 && report.analysed < 20 && (
                  <p className="style-note">
                    Small sample: fewer than 20 processed games (OpenFile
                    disclosure, not a recovered native cutoff).
                  </p>
                )}
                <p>
                  {report.selected} sampled from {report.eligible} eligible ·{" "}
                  {date(report.firstDate)} to {date(report.lastDate)}
                </p>
                <p>{criteriaText}</p>
                <p className="style-availability">
                  {report.dimensions.filter((d) => d.rawScore != null).length}{" "}
                  of {report.dimensions.length} dimensions have a computed
                  score. Missing native inputs and categorical grades remain
                  unavailable.
                </p>
                <p>
                  Identities:{" "}
                  {report.players
                    .map((p) => `${p.name ?? "Unknown"} (#${p.id})`)
                    .join("; ")}
                </p>
                <p>
                  {report.deleted} deleted · {report.filtered} outside filters ·{" "}
                  {report.tooShort} shorter than five stored moves ·{" "}
                  {report.unknownCategory} unknown categories before sampling ·{" "}
                  {report.skipped} skipped · {report.cutGames} cut at 400 plies
                  · {(report.elapsedMs / 1000).toFixed(2)} s
                </p>
                <p className="style-note">
                  Before filters: {report.unknownDate ?? 0} unknown dates ·{" "}
                  {report.unknownOpponentElo ?? 0} unknown opponent ratings ·{" "}
                  {report.unknownResult ?? 0} unknown results. Unknown results
                  remain in the native result denominators and contribute zero
                  points; sample inputs are listed on each computed dimension.
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
                  ["dimensions", "Style dimensions"],
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
                  {report.dimensions.map((dimension) => (
                    <Dimension
                      key={dimension.id}
                      dimension={dimension}
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
                          <th>Stored moves</th>
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
                            <td>{g.storedMoves ?? "—"}</td>
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
                The eight dimensions follow the recovered ChessBase report
                structure. Only calculations with established inputs are shown.
                Missing native repertoire, positional or evaluation inputs
                remain unavailable. Normalized values are composite scores, not
                percentages of games or player percentiles. Exact categorical
                grading is not verified.
              </p>
            </>
          )}
        </div>
        <footer className="modal-actions">
          <span className="style-note">
            Recovered native rules · unavailable inputs disclosed · no result
            cache
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
              <button type="submit" disabled={gameTypes === 0}>
                {report ? "Generate again" : "Generate report"}
              </button>
            )}
          </div>
        </footer>
      </form>
    </div>
  );
}
