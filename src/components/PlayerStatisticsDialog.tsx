import { useMemo, useState } from "react";
import type { PlayerStatistics, StatGroup, StatScore } from "../api/types";

/** ECO index 1..500 of "A00".."E99"; 0 for none. */
const ecoIndex = (eco?: string) =>
  eco ? (eco.charCodeAt(0) - 65) * 100 + Number(eco.slice(1)) + 1 : 0;
const pct = (s: StatScore) => (s.percent == null ? "" : `${s.percent}%`);
const points = (p: number) =>
  Number.isInteger(p) ? String(p) : `${Math.floor(p) || ""}½`;

type OpponentSort = "n" | "az" | "score";
type TournamentSort = "score" | "az" | "date";
type OpeningSort = "white" | "black" | "az";

/** A row of sort buttons, as under each of the reference app's three lists. */
function SortButtons<T extends string>({
  label,
  value,
  options,
  onChange,
  children,
}: {
  label: string;
  value: T;
  options: [T, string][];
  onChange: (v: T) => void;
  children?: React.ReactNode;
}) {
  return (
    <div className="stat-sort" role="group" aria-label={`Sort ${label}`}>
      {options.map(([id, text]) => (
        <button
          key={id}
          type="button"
          aria-pressed={value === id}
          onClick={() => onChange(id)}
        >
          {text}
        </button>
      ))}
      {children}
    </div>
  );
}

/**
 * A player's statistics (the reference app's Player statistic, dialog 846): the
 * score by colour and by year, then three lists side by side, opponents,
 * tournaments and openings, each with the reference app's sort buttons. Double-click
 * a row to list its games.
 */
export function PlayerStatisticsDialog({
  stats,
  progress,
  error,
  players,
  onPlayer,
  onOpenGames,
  onCrosstable,
  onCancel,
  onClose,
}: {
  stats?: PlayerStatistics;
  /** Shown while the games are read. */
  progress?: string;
  error?: string;
  /** The players of the shown game, to switch between them. */
  players?: { id: number; name?: string }[];
  onPlayer: (id: number) => void;
  onOpenGames: (group: StatGroup, label: string) => void;
  onCrosstable: (tournamentId: number) => void;
  onCancel: () => void;
  onClose: () => void;
}) {
  const [opponentSort, setOpponentSort] = useState<OpponentSort>("n");
  const [tournamentSort, setTournamentSort] = useState<TournamentSort>("date");
  const [openingSort, setOpeningSort] = useState<OpeningSort>("az");
  const [chosenEvent, setChosenEvent] = useState<number>();
  const name = stats?.name ?? "?";

  const opponents = useMemo(() => {
    const rows = [...(stats?.opponents ?? [])];
    if (opponentSort === "az")
      rows.sort((a, b) => (a.name ?? "").localeCompare(b.name ?? ""));
    if (opponentSort === "score")
      rows.sort(
        (a, b) =>
          (b.score.percent ?? -1) - (a.score.percent ?? -1) ||
          b.score.games - a.score.games,
      );
    return rows;
  }, [stats, opponentSort]);
  const tournaments = useMemo(() => {
    const rows = [...(stats?.tournaments ?? [])];
    if (tournamentSort === "az")
      rows.sort((a, b) => (a.name ?? "").localeCompare(b.name ?? ""));
    if (tournamentSort === "score")
      rows.sort(
        (a, b) =>
          (b.score.percent ?? -1) - (a.score.percent ?? -1) ||
          b.score.games - a.score.games,
      );
    return rows;
  }, [stats, tournamentSort]);
  const openings = useMemo(() => {
    const rows = [...(stats?.openings ?? [])];
    if (openingSort === "white")
      rows.sort((a, b) => b.white.games - a.white.games);
    if (openingSort === "black")
      rows.sort((a, b) => b.black.games - a.black.games);
    return rows;
  }, [stats, openingSort]);

  const open = (group: StatGroup, label: string) =>
    onOpenGames(group, `${name}: ${label}`);
  const summary = (label: string, s: StatScore, group: StatGroup) => (
    <tr key={label} onDoubleClick={() => open(group, label)}>
      <th scope="row">{label}</th>
      <td className="num">{s.games}</td>
      <td className="num">{s.wins}</td>
      <td className="num">{s.draws}</td>
      <td className="num">{s.losses}</td>
      <td className="num">{points(s.points)}</td>
      <td className="num strong">{pct(s)}</td>
      <td className="num">{s.averageElo ?? ""}</td>
      <td className="num">{s.performance ?? ""}</td>
    </tr>
  );

  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Player statistic"
        className="modal player-statistics"
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <header className="crosstable-head">
          <div>
            <strong>Statistics for {name}</strong>
            {stats && (
              <div className="hint">
                {stats.found} games
                {stats.firstYear
                  ? ` from ${stats.firstYear}${stats.lastYear !== stats.firstYear ? `–${stats.lastYear}` : ""}`
                  : ""}
                {stats.unscored
                  ? ` · ${stats.unscored} without a result, not scored`
                  : ""}
                {stats.deleted ? ` · ${stats.deleted} deleted, left out` : ""}
                {stats.truncated ? " · stopped at the limit" : ""}
              </div>
            )}
          </div>
          {players && players.length > 1 && (
            <div className="segmented" role="group" aria-label="Player">
              {players.map((p, i) => (
                <button
                  key={p.id}
                  type="button"
                  aria-pressed={stats?.playerId === p.id}
                  onClick={() => onPlayer(p.id)}
                >
                  {i === 0 ? "White" : "Black"}: {p.name ?? "?"}
                </button>
              ))}
            </div>
          )}
        </header>
        {progress && (
          <div className="empty-state">
            {progress}{" "}
            <button type="button" className="link-button" onClick={onCancel}>
              Cancel
            </button>
          </div>
        )}
        {error && <div className="empty-state">{error}</div>}
        {stats && !progress && (
          <div className="stat-body">
            <div className="stat-top">
              <table className="stat-table" aria-label="Score by colour">
                <thead>
                  <tr>
                    <th />
                    <th className="num" title="Games">
                      N
                    </th>
                    <th className="num">+</th>
                    <th className="num">=</th>
                    <th className="num">−</th>
                    <th className="num">Pts.</th>
                    <th className="num">Score</th>
                    <th className="num" title="Average Elo of the opponents">
                      Elo-Av
                    </th>
                    <th className="num">Perf.</th>
                  </tr>
                </thead>
                <tbody>
                  {summary("White", stats.white, {
                    kind: "color",
                    white: true,
                  })}
                  {summary("Black", stats.black, {
                    kind: "color",
                    white: false,
                  })}
                  {summary("Total", stats.total, { kind: "all" })}
                </tbody>
              </table>
              <div className="stat-scroll stat-years">
                <table className="stat-table" aria-label="Years">
                  <thead>
                    <tr>
                      <th>Year</th>
                      <th className="num">Elo</th>
                      <th className="num">N</th>
                      <th className="num">Score</th>
                      <th className="num">Perf.</th>
                    </tr>
                  </thead>
                  <tbody>
                    {stats.years.map((y) => (
                      <tr
                        key={y.year}
                        onDoubleClick={() =>
                          open(
                            { kind: "year", year: y.year },
                            y.year ? String(y.year) : "no year",
                          )
                        }
                      >
                        <td>{y.year || "?"}</td>
                        <td className="num">{y.elo ?? ""}</td>
                        <td className="num">{y.score.games}</td>
                        <td className="num">{pct(y.score)}</td>
                        <td className="num">{y.score.performance ?? ""}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
            <div className="stat-lists">
              <section aria-label="Opponents">
                <div className="stat-scroll">
                  <table className="stat-table">
                    <thead>
                      <tr>
                        <th>Opponent</th>
                        <th className="num">Elo</th>
                        <th className="num">N</th>
                        <th className="num">Score</th>
                      </tr>
                    </thead>
                    <tbody>
                      {opponents.map((o) => (
                        <tr
                          key={o.id}
                          onDoubleClick={() =>
                            open(
                              { kind: "opponent", id: o.id },
                              `against ${o.name ?? "?"}`,
                            )
                          }
                        >
                          <td>{o.name ?? "?"}</td>
                          <td className="num">{o.elo ?? ""}</td>
                          <td className="num">{o.score.games}</td>
                          <td className="num">{pct(o.score)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  {stats.moreOpponents > 0 && (
                    <div className="hint">
                      {stats.moreOpponents} more opponents not shown
                    </div>
                  )}
                </div>
                <SortButtons
                  label="opponents"
                  value={opponentSort}
                  onChange={setOpponentSort}
                  options={[
                    ["n", "N"],
                    ["az", "A-Z"],
                    ["score", "Score"],
                  ]}
                />
              </section>
              <section aria-label="Tournaments">
                <div className="stat-scroll">
                  <table className="stat-table">
                    <thead>
                      <tr>
                        <th>Tournament</th>
                        <th className="num">Year</th>
                        <th className="num">N</th>
                        <th className="num">Score</th>
                        <th className="num">Perf.</th>
                      </tr>
                    </thead>
                    <tbody>
                      {tournaments.map((t) => (
                        <tr
                          key={t.id}
                          aria-selected={chosenEvent === t.id}
                          onClick={() => setChosenEvent(t.id)}
                          onDoubleClick={() =>
                            open(
                              { kind: "tournament", id: t.id },
                              t.name ?? "?",
                            )
                          }
                        >
                          <td title={t.site}>
                            {t.name ?? "?"}
                            {t.site ? (
                              <span className="hint"> {t.site}</span>
                            ) : null}
                          </td>
                          <td className="num">{t.year || ""}</td>
                          <td className="num">{t.score.games}</td>
                          <td className="num">{pct(t.score)}</td>
                          <td className="num">{t.score.performance ?? ""}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  {stats.moreTournaments > 0 && (
                    <div className="hint">
                      {stats.moreTournaments} more tournaments not shown
                    </div>
                  )}
                </div>
                <SortButtons
                  label="tournaments"
                  value={tournamentSort}
                  onChange={setTournamentSort}
                  options={[
                    ["score", "Score"],
                    ["az", "A-Z"],
                    ["date", "Date"],
                  ]}
                >
                  <button
                    type="button"
                    disabled={chosenEvent === undefined}
                    title="Cross table of the chosen tournament"
                    onClick={() =>
                      chosenEvent !== undefined && onCrosstable(chosenEvent)
                    }
                  >
                    Table
                  </button>
                </SortButtons>
              </section>
              <section aria-label="Openings">
                <div className="stat-scroll">
                  <table className="stat-table">
                    <thead>
                      <tr>
                        <th>ECO</th>
                        <th className="num" title="Games with White">
                          White
                        </th>
                        <th className="num">%</th>
                        <th className="num" title="Games with Black">
                          Black
                        </th>
                        <th className="num">%</th>
                      </tr>
                    </thead>
                    <tbody>
                      {openings.map((o) => (
                        <tr
                          key={o.eco ?? ""}
                          onDoubleClick={() =>
                            open(
                              { kind: "opening", eco: ecoIndex(o.eco) },
                              o.eco ?? "no ECO",
                            )
                          }
                        >
                          <td>{o.eco ?? "—"}</td>
                          <td className="num">{o.white.games || ""}</td>
                          <td className="num">
                            {o.white.games ? pct(o.white) : ""}
                          </td>
                          <td className="num">{o.black.games || ""}</td>
                          <td className="num">
                            {o.black.games ? pct(o.black) : ""}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
                <SortButtons
                  label="openings"
                  value={openingSort}
                  onChange={setOpeningSort}
                  options={[
                    ["white", "White"],
                    ["black", "Black"],
                    ["az", "A-Z"],
                  ]}
                />
              </section>
            </div>
          </div>
        )}
        <div className="modal-actions">
          <span className="hint">Double-click a row to list its games.</span>
          <button type="button" autoFocus onClick={onClose}>
            OK
          </button>
        </div>
      </div>
    </div>
  );
}
