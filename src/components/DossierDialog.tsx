import type {
  ColourStats,
  Dossier,
  DossierGame,
  RepertoireRow,
  StatGroup,
} from "../api/types";

/**
 * The workbench's words for ChessBase's bands; the boundaries are
 * ChessBase's, the words are not.
 */
const scoreWords = [
  "far below expectation",
  "clearly below expectation",
  "slightly below expectation",
  "as expected",
  "slightly above expectation",
  "clearly above expectation",
  "far above expectation",
];
const lengthWords = [
  "very short",
  "short",
  "somewhat short",
  "of usual length",
  "somewhat long",
  "long",
  "very long",
];
const drawWords = [
  "very low",
  "low",
  "somewhat low",
  "usual",
  "somewhat high",
  "high",
];
const word = (words: string[], band?: number) =>
  band == null ? undefined : words[band];
const points = (p: number) =>
  Number.isInteger(p) ? String(p) : `${Math.floor(p) || ""}½`;

/** The sentences of a colour, as ChessBase's dossier writes them. */
function colourSentences(label: string, s: ColourStats) {
  const out: string[] = [];
  const verdict = word(scoreWords, s.scoreBand);
  if (s.score != null && verdict)
    out.push(`${label}: the score is ${verdict} (${s.score}%).`);
  if (s.performance != null && s.opponentsAverage != null) {
    const d = s.performance - s.opponentsAverage;
    out.push(
      `${label}: performance ${s.performance} against an average opposition of ${s.opponentsAverage} (${d > 0 ? "+" : ""}${d}).`,
    );
  }
  return out;
}

function lengthSentences(s: ColourStats) {
  const out: string[] = [];
  const add = (what: string, moves?: number, band?: number) => {
    const w = word(lengthWords, band);
    if (moves != null && w) out.push(`${what} are ${w} (${moves} moves).`);
  };
  add("White's wins", s.whiteWinMoves, s.whiteWinBand);
  add("Draws", s.drawMoves, s.drawBand);
  add("Black's wins", s.blackWinMoves, s.blackWinBand);
  const d = word(drawWords, s.drawQuoteBand);
  if (s.drawPercent != null && d)
    out.push(
      `The share of draws is ${d} (${s.drawPercent}%), quick draws ${s.quickDrawPercent ?? 0}%.`,
    );
  return out;
}

function GameTable({
  label,
  games,
  count,
  onLoadGame,
}: {
  label: string;
  games: DossierGame[];
  count: number;
  onLoadGame: (ordinal: string) => void;
}) {
  return (
    <section aria-label={label}>
      <h3>
        {label} <span className="hint">{count}</span>
      </h3>
      {games.length === 0 ? (
        <p className="hint">None.</p>
      ) : (
        <table className="stat-table">
          <thead>
            <tr>
              <th>Opponent</th>
              <th>Colour</th>
              <th>Result</th>
              <th className="num">Moves</th>
              <th>ECO</th>
              <th>Event</th>
              <th className="num">Year</th>
              {games.some((g) => g.medals.length > 0) && <th>Medals</th>}
            </tr>
          </thead>
          <tbody>
            {games.map((g) => (
              <tr
                key={g.ordinal}
                title="Double-click to load this game"
                onDoubleClick={() => onLoadGame(g.ordinal)}
              >
                <td>{g.opponent ?? "?"}</td>
                <td>{g.white ? "White" : "Black"}</td>
                <td>{g.result ?? ""}</td>
                <td className="num">{g.moves}</td>
                <td>{g.eco ?? ""}</td>
                <td>{g.tournament ?? ""}</td>
                <td className="num">{g.year ?? ""}</td>
                {games.some((x) => x.medals.length > 0) && (
                  <td>{g.medals.join(", ")}</td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {count > games.length && (
        <p className="hint">The latest {games.length} are listed.</p>
      )}
    </section>
  );
}

function Repertoire({ label, rows }: { label: string; rows: RepertoireRow[] }) {
  return (
    <section aria-label={label}>
      <h3>{label}</h3>
      {rows.length === 0 ? (
        <p className="hint">No line is played often enough.</p>
      ) : (
        <table className="stat-table">
          <thead>
            <tr>
              <th>Line</th>
              <th>Opening</th>
              <th className="num">N</th>
              <th className="num">Score</th>
              <th className="num">Last</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((l) => (
              <tr key={l.moves.join(" ")}>
                <td className="prepare-line">{l.text}</td>
                <td>{[l.eco, l.name].filter(Boolean).join(" ")}</td>
                <td className="num">{l.games}</td>
                <td className="num">
                  {l.score == null ? "" : `${l.score.toFixed(0)}%`}
                </td>
                <td className="num">{l.lastYear ?? ""}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

/**
 * A player's dossier (ChessBase's `PLAYERDOSSIER`, Ctrl+Alt+D), from the
 * open database: the statistics, years, opponents, tournaments, repertoire
 * and notable games that ChessBase's dossier writes. ChessBase's
 * biography and pictures need its player encyclopedia and are left out.
 */
export function DossierDialog({
  name,
  data,
  progress,
  error,
  onCancel,
  onOpenGames,
  onLoadGame,
  onClose,
}: {
  name?: string;
  data?: Dossier;
  progress?: string;
  error?: string;
  onCancel: () => void;
  onOpenGames: (group: StatGroup, label: string) => void;
  onLoadGame: (ordinal: string) => void;
  onClose: () => void;
}) {
  const who = data?.name ?? name ?? "?";
  const open = (group: StatGroup, label: string) =>
    onOpenGames(group, `${who}: ${label}`);
  const sentences = data
    ? [
        ...colourSentences("With White", data.white),
        ...colourSentences("With Black", data.black),
        ...lengthSentences(data.all),
      ]
    : [];
  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Dossier"
        className="modal dossier"
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <header className="crosstable-head">
          <div>
            <strong>Dossier: {who}</strong>
            {data && (
              <div className="hint">
                {data.games} {data.games === 1 ? "game" : "games"}
                {data.firstYear
                  ? ` from ${data.firstYear}${data.lastYear !== data.firstYear ? `–${data.lastYear}` : ""}`
                  : ""}
                {data.deleted ? ` · ${data.deleted} deleted, left out` : ""}
                {data.truncated ? " · stopped at the limit" : ""}
              </div>
            )}
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
          <div className="dossier-body">
            <section aria-label="Statistics">
              <h3>Statistics</h3>
              <table className="stat-table dossier-score">
                <thead>
                  <tr>
                    <th />
                    <th className="num">N</th>
                    <th className="num" title="White wins">
                      1-0
                    </th>
                    <th className="num">½</th>
                    <th className="num" title="Black wins">
                      0-1
                    </th>
                    <th className="num">Score</th>
                    <th className="num">Elo-Av</th>
                    <th className="num">Perf.</th>
                  </tr>
                </thead>
                <tbody>
                  {(
                    [
                      ["White", data.white, { kind: "color", white: true }],
                      ["Black", data.black, { kind: "color", white: false }],
                      ["Total", data.all, { kind: "all" }],
                    ] as [string, ColourStats, StatGroup][]
                  ).map(([label, s, group]) => (
                    <tr key={label} onDoubleClick={() => open(group, label)}>
                      <th scope="row">{label}</th>
                      <td className="num">{s.games}</td>
                      <td className="num">{s.whiteWins}</td>
                      <td className="num">{s.draws}</td>
                      <td className="num">{s.blackWins}</td>
                      <td className="num strong">
                        {s.score == null ? "" : `${s.score}%`}
                      </td>
                      <td className="num">{s.opponentsAverage ?? ""}</td>
                      <td className="num">{s.performance ?? ""}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              {sentences.map((t) => (
                <p key={t}>{t}</p>
              ))}
            </section>
            <div className="dossier-columns">
              <section aria-label="Elo profile">
                <h3>Elo profile and games played in years</h3>
                <table className="stat-table">
                  <thead>
                    <tr>
                      <th>Year</th>
                      <th className="num">Elo</th>
                      <th className="num">N</th>
                      <th className="num">Score</th>
                    </tr>
                  </thead>
                  <tbody>
                    {data.years.map((y) => (
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
                        <td className="num">
                          {y.score.percent == null ? "" : `${y.score.percent}%`}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </section>
              <section aria-label="Score by rating">
                <h3>Score by the opponent's rating</h3>
                {data.ratingBands.length === 0 && (
                  <p className="hint">No opponent is rated.</p>
                )}
                <table className="stat-table">
                  <thead>
                    <tr>
                      <th>Elo</th>
                      <th className="num">N</th>
                      <th className="num">Pts.</th>
                      <th className="num">Score</th>
                    </tr>
                  </thead>
                  <tbody>
                    {data.ratingBands.map((b) => (
                      <tr key={b.from}>
                        <td>
                          {b.from}–{b.from + 49}
                        </td>
                        <td className="num">{b.games}</td>
                        <td className="num">{points(b.points)}</td>
                        <td className="num">{b.percent}%</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </section>
            </div>
            <div className="dossier-columns">
              <section aria-label="Tournaments">
                <h3>Tournaments</h3>
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
                    {data.tournaments.map((t) => (
                      <tr
                        key={t.id}
                        onDoubleClick={() =>
                          open({ kind: "tournament", id: t.id }, t.name ?? "?")
                        }
                      >
                        <td>{t.name ?? "?"}</td>
                        <td className="num">{t.year || ""}</td>
                        <td className="num">{t.score.games}</td>
                        <td className="num">
                          {t.score.percent == null ? "" : `${t.score.percent}%`}
                        </td>
                        <td className="num">{t.score.performance ?? ""}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                {data.moreTournaments > 0 && (
                  <p className="hint">{data.moreTournaments} more.</p>
                )}
              </section>
              <section aria-label="Opponents">
                <h3>Opponents</h3>
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
                    {data.opponents.map((o) => (
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
                        <td className="num">
                          {o.score.percent == null ? "" : `${o.score.percent}%`}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                {data.moreOpponents > 0 && (
                  <p className="hint">{data.moreOpponents} more.</p>
                )}
              </section>
            </div>
            <div className="dossier-columns">
              <Repertoire
                label="Repertoire for White"
                rows={data.repertoireWhite}
              />
              <Repertoire
                label="Repertoire for Black"
                rows={data.repertoireBlack}
              />
            </div>
            <GameTable
              label="Games with medals"
              games={data.medalGames}
              count={data.medalCount}
              onLoadGame={onLoadGame}
            />
            <GameTable
              label="Mates given"
              games={data.matesGiven}
              count={data.matesGivenCount}
              onLoadGame={onLoadGame}
            />
            <GameTable
              label="Mates received"
              games={data.matesReceived}
              count={data.matesReceivedCount}
              onLoadGame={onLoadGame}
            />
            <GameTable
              label="Annotated by the player"
              games={data.annotated}
              count={data.annotatedCount}
              onLoadGame={onLoadGame}
            />
            <p className="hint">
              ChessBase's biography, pictures and rating history come from its
              player encyclopedia and are not part of this dossier.
            </p>
          </div>
        )}
        <div className="modal-actions">
          <span className="hint">
            Double-click a row to list its games, or a game to load it.
          </span>
          <button type="button" autoFocus onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
