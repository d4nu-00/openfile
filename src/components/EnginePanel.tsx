import {
  Crosshair,
  History,
  Lock,
  Minus,
  Plus,
  SkipForward,
} from "lucide-react";
import { formatScore } from "../state/analysis";
import {
  scoreCp,
  type KibitzerView,
  type useKibitzers,
} from "../state/kibitzers";
import type { WorkbenchState } from "../state/workbench";
import { EngineOptions } from "./EngineOptions";

/** One kibitzer: its engine, controls and MultiPV lines. */
function KibitzerPane({
  k,
  view,
  state,
  active,
}: {
  k: ReturnType<typeof useKibitzers>;
  view: KibitzerView;
  state: WorkbenchState;
  active: boolean;
}) {
  const name =
    view.engine.identity.name ?? view.installed?.name ?? "UCI engine";
  return (
    <section
      className="kibitzer"
      aria-label={`Kibitzer ${name}`}
      data-active={active}
      onPointerDown={() => k.setActive(view.key)}
    >
      <div className="engine-controls">
        <strong>{name}</strong>
        <button
          disabled={!state.document?.documentId || view.busy}
          onClick={() =>
            view.enabled ? void k.stop(view.key) : k.start(view.key)
          }
        >
          {view.enabled ? "Stop analysis" : "Start analysis"}
        </button>
        <span className="lines-control" title="Lines (MultiPV)">
          <button
            aria-label="Fewer lines"
            disabled={view.busy || view.multipv <= 1}
            onClick={() => void k.lines(-1, view.key)}
          >
            <Minus size={12} aria-hidden />
          </button>
          <span aria-label="Lines">{view.multipv}</span>
          <button
            aria-label="More lines"
            disabled={view.busy}
            onClick={() => void k.lines(1, view.key)}
          >
            <Plus size={12} aria-hidden />
          </button>
        </span>
        <button disabled={view.busy} onClick={() => void k.remove(view.key)}>
          Close engine
        </button>
        <span role="status">{view.status}</span>
      </div>
      {view.error && <p role="alert">{view.error}</p>}
      <SearchMode k={k} view={view} />
      <EngineOptions
        options={view.engine.options}
        disabled={view.busy}
        apply={(n, v) => k.option(view.key, n, v)}
      />
      {view.showHistory ? (
        <HistoryTable view={view} />
      ) : (
        <table aria-label="Engine analysis">
          <thead>
            <tr>
              <th>Line</th>
              <th>Depth</th>
              <th>Nodes</th>
              <th>Score</th>
              <th>Principal variation</th>
            </tr>
          </thead>
          <tbody>
            {view.lines.map((line) => (
              <tr key={line.multipv}>
                <td>{line.multipv}</td>
                <td>{line.depth ?? "—"}</td>
                <td>{line.nodes ?? "—"}</td>
                <td>
                  {line.score
                    ? formatScore(line.score, line.rootTurn, "white")
                    : "—"}
                </td>
                <td>
                  {line.pv?.join(" ")}
                  {line.diagnostics.map((d, i) => (
                    <div role="alert" key={i}>
                      {d}
                    </div>
                  ))}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

/** "0.8 s", "12.4 s", "3:05". */
function elapsed(ms: number) {
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)} s`;
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** The engine's choices for the position, latest first. */
function HistoryTable({ view }: { view: KibitzerView }) {
  return (
    <table aria-label="Engine history">
      <thead>
        <tr>
          <th>Time</th>
          <th>Depth</th>
          <th>Score</th>
          <th>Best line</th>
        </tr>
      </thead>
      <tbody>
        {[...view.history].reverse().map((h, i) => (
          <tr key={`${h.move}-${h.time}`} data-current={i === 0}>
            <td>{elapsed(h.time)}</td>
            <td>{h.depth ?? "—"}</td>
            <td>{h.score ? formatScore(h.score, h.rootTurn, "white") : "—"}</td>
            <td>
              <strong>{h.pv[0] ?? h.move}</strong> {h.pv.slice(1).join(" ")}
            </td>
          </tr>
        ))}
        {!view.history.length && (
          <tr>
            <td colSpan={4}>No choice yet.</td>
          </tr>
        )}
      </tbody>
    </table>
  );
}

/**
 * Analyse Threat, Next Best / Search Better and Lock Engine: buttons and what
 * the kibitzer is doing with them.
 */
function SearchMode({
  k,
  view,
}: {
  k: ReturnType<typeof useKibitzers>;
  view: KibitzerView;
}) {
  const best = view.lines.find((l) => l.multipv === 1);
  const last = view.excluded.at(-1);
  const target = view.target;
  const better =
    target && best?.score ? scoreCp(best.score) > target.cp : undefined;
  const side = best?.rootTurn === "black" ? "Black" : "White";
  return (
    <div className="engine-mode" aria-label="Engine search">
      <button
        type="button"
        aria-pressed={view.threat}
        disabled={!view.enabled || view.busy}
        title="Analyse threat (X)"
        onClick={() => k.toggleThreat(view.key)}
      >
        <Crosshair size={12} aria-hidden /> Threat
      </button>
      <button
        type="button"
        disabled={!view.enabled || !view.lines.length}
        title="Next Best (Y)"
        onClick={() => k.leaveOutBest("next", view.key)}
      >
        <SkipForward size={12} aria-hidden /> Next best
      </button>
      <button
        type="button"
        aria-pressed={view.showHistory}
        title="Show how the engine's choice changed"
        onClick={() => k.toggleHistory(view.key)}
      >
        <History size={12} aria-hidden /> History
      </button>
      <button
        type="button"
        aria-pressed={!!view.locked}
        disabled={!view.enabled}
        title="Lock Engine (Ctrl+Alt+L)"
        onClick={() => k.toggleLock(view.key)}
      >
        <Lock size={12} aria-hidden /> Lock
      </button>
      {view.locked && (
        <span className="engine-badge">Locked on {view.locked.label}</span>
      )}
      {view.threat && best && (
        <span className="engine-badge">Threat: {side} moves again</span>
      )}
      {view.excluded.length > 0 && (
        <span aria-label="Left out">
          Left out:{" "}
          {view.excluded
            .map(
              (e) =>
                e.san +
                (e.score
                  ? ` (${formatScore(e.score, e.rootTurn, "white")})`
                  : ""),
            )
            .join(", ")}
        </span>
      )}
      {target && last && (
        <span role="status" aria-label="Search Better">
          {better
            ? `Found a move that scores more than ${formatScore({ kind: "cp", value: target.cp }, last.rootTurn, "white")}`
            : `Looking for a move that scores more than ${formatScore({ kind: "cp", value: target.cp }, last.rootTurn, "white")}`}
        </span>
      )}
    </div>
  );
}

/** The engine pane: one kibitzer after another. */
export function EnginePanel({
  k,
  state,
}: {
  k: ReturnType<typeof useKibitzers>;
  state: WorkbenchState;
}) {
  return (
    <>
      <div className="engine-controls">
        <h3>Analysis</h3>
        <button disabled={k.adding} onClick={() => void k.choose()}>
          Add kibitzer
        </button>
        {!k.kibitzers.length && (
          <span role="status">Add a local UCI engine.</span>
        )}
      </div>
      {k.error && <p role="alert">{k.error}</p>}
      <p className="score-perspective">Score from White's perspective</p>
      {k.kibitzers.map((view) => (
        <KibitzerPane
          key={view.key}
          k={k}
          view={view}
          state={state}
          active={view.key === k.current?.key}
        />
      ))}
    </>
  );
}
