import type { OpeningReport, ReportGame, ReportPlayer } from "../api/types";

/** SAN moves numbered from a start position given by its FEN, `skip`
 * plies after it. */
export function numbered(moves: string[], startFen?: string, skip = 0) {
  const fields = startFen?.split(" ") ?? [];
  let black = fields[1] === "b";
  let n = Number(fields[5]) || 1;
  for (let i = 0; i < skip; i++) {
    if (black) n += 1;
    black = !black;
  }
  const out: string[] = [];
  moves.forEach((m, i) => {
    if (!black) out.push(`${n}.${m}`);
    else out.push(i === 0 ? `${n}...${m}` : m);
    if (black) n += 1;
    black = !black;
  });
  return out.join(" ");
}

const pct = (v?: number) => (v === undefined ? "–" : `${v.toFixed(1)}%`);

function GameRow({ label, game }: { label: string; game?: ReportGame }) {
  if (!game) return null;
  const elo = (e?: number) => (e ? ` ${e}` : "");
  return (
    <tr>
      <th scope="row">{label}</th>
      <td>
        {game.white ?? "?"}
        {elo(game.whiteElo)} – {game.black ?? "?"}
        {elo(game.blackElo)}
      </td>
      <td>{game.result ?? ""}</td>
      <td>{game.date ?? ""}</td>
      <td>{game.event ?? ""}</td>
    </tr>
  );
}

function Players({ title, rows }: { title: string; rows: ReportPlayer[] }) {
  if (!rows.length) return null;
  return (
    <>
      <h4>{title}</h4>
      <table className="report-table">
        <thead>
          <tr>
            <th>Player</th>
            <th>Games</th>
            <th>Score</th>
            <th>Elo</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((p) => (
            <tr key={p.name}>
              <td>{p.name}</td>
              <td>{p.games}</td>
              <td>{pct(p.score)}</td>
              <td>{p.bestElo ?? p.averageElo ?? ""}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

/** A small bar chart of `[label, value]` pairs. */
function Bars({
  title,
  bars,
  format,
  cap,
}: {
  title: string;
  bars: [string, number][];
  format?: (v: number) => string;
  /** Values above it are drawn at it (the tooltip keeps the value). */
  cap?: number;
}) {
  const max = Math.max(
    1,
    ...bars.map((b) => (cap ? Math.min(b[1], cap) : b[1])),
  );
  return (
    <figure className="report-bars" aria-label={title}>
      <figcaption>{title}</figcaption>
      <div className="report-bars-plot">
        {bars.map(([label, v]) => (
          <div
            key={label}
            className="report-bar"
            title={`${label}: ${format ? format(v) : v}`}
            style={{ height: `${(Math.min(v, cap ?? v) / max) * 100}%` }}
          />
        ))}
      </div>
      <div className="report-bars-axis">
        <span>{bars[0]?.[0]}</span>
        <span>{bars[bars.length - 1]?.[0]}</span>
      </div>
    </figure>
  );
}

/**
 * Opening Report: what the database says about the board position, after
 * The reference app's report (title, history, players, moves, main line).
 */
export function OpeningReportDialog({
  report,
  startFen,
  running,
  error,
  onStop,
  onClose,
}: {
  report?: OpeningReport;
  startFen?: string;
  /** Progress while the report is made. */
  running?: string;
  error?: string;
  onStop: () => void;
  onClose: () => void;
}) {
  const r = report;
  const decided = r ? r.whiteWins + r.draws + r.blackWins : 0;
  const share = (n: number) =>
    decided ? `${((n * 100) / decided).toFixed(1)}%` : "–";
  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Opening Report"
        className="modal opening-report"
        onKeyDown={(e) => e.key === "Escape" && !running && onClose()}
      >
        <h2>
          Opening Report
          {r?.eco ? `: ${r.eco}` : ""}
          {r?.name ? ` ${r.name}` : ""}
        </h2>
        {running && (
          <p role="status" className="hint">
            {running}
          </p>
        )}
        {error && <p role="alert">{error}</p>}
        {r && !r.cancelled && (
          <div className="report-body">
            <p>{numbered(r.line, startFen) || "Start position"}</p>
            {r.games === 0 ? (
              <p>No game of the database reaches this position.</p>
            ) : (
              <>
                <p>
                  {r.games.toLocaleString()} games
                  {r.truncated
                    ? " (the first 100,000 in database order; more reach the position)"
                    : ""}
                  : White wins {share(r.whiteWins)}, draws {share(r.draws)},
                  Black wins {share(r.blackWins)}.
                </p>
                <h3>History</h3>
                <table className="report-table">
                  <tbody>
                    {r.listed.map((g, i) => (
                      <GameRow key={g.ordinal} label={`${i + 1}.`} game={g} />
                    ))}
                    <GameRow label="Earliest game" game={r.earliest} />
                    <GameRow
                      label="Latest annotated game"
                      game={r.latestAnnotated}
                    />
                    <GameRow label="Latest strong game" game={r.latestStrong} />
                    <GameRow label="Latest game" game={r.latest} />
                  </tbody>
                </table>
                {r.years.length > 1 && (
                  <Bars
                    title="Games played in years"
                    bars={r.years.map(([y, n]) => [String(y), n])}
                  />
                )}
                {r.fashion.length > 1 && (
                  <Bars
                    title="Fashion index: share of games relative to the database (100 = average)"
                    bars={r.fashion.map((b) => [
                      b.from === b.to ? String(b.from) : `${b.from}–${b.to}`,
                      b.index ?? 0,
                    ])}
                    format={(v) => v.toFixed(0)}
                    cap={400}
                  />
                )}
                <h3>Players</h3>
                <Players
                  title="Strong players who used this line as White"
                  rows={r.strongWhite}
                />
                <Players
                  title="Strong players who used this line as Black"
                  rows={r.strongBlack}
                />
                <Players title="Other notable players" rows={r.notable} />
                <h3>Moves</h3>
                <table className="report-table">
                  <thead>
                    <tr>
                      <th>Move</th>
                      <th>N</th>
                      <th>%</th>
                      <th>Score</th>
                      <th>Draws</th>
                      <th>Av</th>
                      <th>Perf</th>
                      <th>Last</th>
                      <th>Played by</th>
                    </tr>
                  </thead>
                  <tbody>
                    {r.moves.map((m) => (
                      <tr key={m.uci}>
                        <td>{m.san}</td>
                        <td>{m.games}</td>
                        <td>{pct(m.share)}</td>
                        <td>{pct(m.score)}</td>
                        <td>{pct(m.draws)}</td>
                        <td>{m.averageElo ?? ""}</td>
                        <td>{m.performance ?? ""}</td>
                        <td>{m.lastYear ?? ""}</td>
                        <td>{m.playedBy.join(", ")}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <p className="hint">
                  {r.movesFromTree
                    ? "Move statistics come from the opening tree, all games included."
                    : `Move statistics come from the ${r.read} strongest games read.`}{" "}
                  "Played by" lists the strongest players among the {r.read}{" "}
                  strongest games.
                </p>
                {r.mainLine.length > 0 && (
                  <>
                    <h3>Main line</h3>
                    <p>{numbered(r.mainLine, startFen, r.line.length)}</p>
                  </>
                )}
                <p className="hint">
                  Not in this report: plans, "You should play" recommendations,
                  critical lines, opening keys and photos.
                </p>
              </>
            )}
          </div>
        )}
        <div className="mask-buttons">
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
