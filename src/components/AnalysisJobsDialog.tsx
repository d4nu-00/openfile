import { useState } from "react";
import type {
  AnalysisJob,
  InstalledEngine,
  JobKind,
  JobsView,
  RunJobsParams,
} from "../api/types";

/** The settings for new jobs; ChessBase's minimums are 15 s and 30 s. */
export const defaultJobKind: JobKind = {
  type: "analysis",
  lines: 3,
  seconds: 30,
};

export function describeKind(kind: JobKind) {
  return kind.type === "analysis"
    ? `Analysis, ${kind.lines} ${kind.lines === 1 ? "line" : "lines"}, ${kind.seconds} s`
    : `Deep Analysis, ${kind.seconds} s`;
}

const statusText: Record<AnalysisJob["status"], string> = {
  waiting: "Waiting",
  running: "Running",
  done: "Done",
  skipped: "Skipped",
  failed: "Failed",
};

/** The move a job's position is after, e.g. "12.Nf3" as "after move 12". */
function where(job: AnalysisJob) {
  const n = job.moves.length;
  return n === 0 ? "start" : `move ${Math.ceil(n / 2)}${n % 2 ? "" : "…"}`;
}

/**
 * The analysis jobs queue: positions of games in a database opened for
 * editing, analysed one after another; each game is replaced with its
 * analysis ("Merge result with original game").
 */
export function AnalysisJobsDialog({
  view,
  engines,
  defaultEngine,
  kind,
  progress,
  onKindChange,
  onChooseFile,
  onRun,
  onStop,
  onEdit,
  onClose,
  error,
}: {
  view: JobsView;
  engines: InstalledEngine[];
  defaultEngine?: string;
  /** The settings jobs added from now on get. */
  kind: JobKind;
  /** "Job 2 of 5" while running. */
  progress?: string;
  onKindChange: (kind: JobKind) => void;
  onChooseFile: () => Promise<string | string[] | null>;
  onRun: (params: RunJobsParams) => void;
  onStop: () => void;
  onEdit: (id: number, action: "restart" | "skip" | "remove") => void;
  onClose: () => void;
  error?: string;
}) {
  const [engine, setEngine] = useState(
    engines.find((e) => e.id === defaultEngine)?.path ?? engines[0]?.path,
  );
  const [chosen, setChosen] = useState<number>();
  const options = engines.find((e) => e.path === engine)?.options ?? {};
  const job = view.jobs.find((j) => j.id === chosen);
  const waiting = view.jobs.some((j) => j.status === "waiting");
  const min = kind.type === "analysis" ? 15 : 30;
  const seconds = (s: string) => {
    const v = Math.round(Number(s));
    return Number.isFinite(v) ? v : min;
  };
  const timeValid =
    kind.seconds >= min &&
    kind.seconds <= (kind.type === "analysis" ? 3600 : 86400);
  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Analysis jobs"
        className="modal analysis-jobs"
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Analysis jobs</h2>
        <div className="jobs-table" role="table" aria-label="Jobs">
          <div role="row" className="jobs-row jobs-head">
            <span role="columnheader">#</span>
            <span role="columnheader">Game</span>
            <span role="columnheader">Position</span>
            <span role="columnheader">Type</span>
            <span role="columnheader">Status</span>
          </div>
          {view.jobs.length === 0 && (
            <div role="row" className="jobs-row">
              <span role="cell" className="hint">
                No jobs yet: select games in an editable database and use Create
                analysis jobs, or Add Analysis Job in the game window.
              </span>
            </div>
          )}
          {view.jobs.map((j, i) => (
            <div
              key={j.id}
              role="row"
              aria-selected={j.id === chosen}
              className={`jobs-row${j.id === chosen ? " selected" : ""}`}
              onClick={() => setChosen(j.id)}
            >
              <span role="cell">{i + 1}</span>
              <span role="cell">
                {j.label} (#{j.ordinal})
              </span>
              <span role="cell">{where(j)}</span>
              <span role="cell">{describeKind(j.kind)}</span>
              <span role="cell" title={j.message}>
                {statusText[j.status]}
                {j.message ? `: ${j.message}` : ""}
              </span>
            </div>
          ))}
        </div>
        <div className="mask-buttons">
          {(["restart", "skip", "remove"] as const).map((action) => (
            <button
              key={action}
              type="button"
              disabled={!job || job.status === "running"}
              onClick={() => job && onEdit(job.id, action)}
            >
              {action === "restart"
                ? "Restart"
                : action === "skip"
                  ? "Skip"
                  : "Remove"}
            </button>
          ))}
        </div>
        <fieldset className="mask-row" disabled={view.running}>
          <legend className="mask-label">New jobs</legend>
          <label className="mask-check">
            <input
              type="radio"
              name="job-type"
              checked={kind.type === "analysis"}
              onChange={() =>
                onKindChange({
                  type: "analysis",
                  lines: 3,
                  seconds: Math.max(15, kind.seconds),
                })
              }
            />
            Analysis
          </label>
          <label className="mask-check">
            <input
              type="radio"
              name="job-type"
              checked={kind.type === "deep"}
              onChange={() =>
                onKindChange({
                  type: "deep",
                  seconds: Math.max(30, kind.seconds),
                })
              }
            />
            Deep Analysis
          </label>{" "}
          <label>
            Time [s]{" "}
            <input
              type="number"
              aria-label="Time"
              min={min}
              step={1}
              value={kind.seconds}
              onChange={(e) =>
                onKindChange({ ...kind, seconds: seconds(e.target.value) })
              }
            />
          </label>{" "}
          {kind.type === "analysis" && (
            <label>
              Lines{" "}
              <input
                type="number"
                aria-label="Lines"
                min={1}
                max={10}
                step={1}
                value={kind.lines}
                onChange={(e) =>
                  onKindChange({
                    ...kind,
                    lines: Math.min(
                      10,
                      Math.max(1, Math.round(Number(e.target.value)) || 1),
                    ),
                  })
                }
              />
            </label>
          )}
          {!timeValid && (
            <p role="alert">
              The time must be at least {min} s, as in ChessBase.
            </p>
          )}
        </fieldset>
        <label className="mask-row">
          <span>Engine</span>{" "}
          <select
            aria-label="Engine"
            value={engine ?? ""}
            disabled={view.running}
            onChange={(e) => setEngine(e.target.value)}
          >
            {!engine && <option value="">Choose an engine</option>}
            {engines.map((e) => (
              <option key={e.id} value={e.path}>
                {e.name}
              </option>
            ))}
            {engine && !engines.some((e) => e.path === engine) && (
              <option value={engine}>{engine}</option>
            )}
          </select>{" "}
          <button
            type="button"
            disabled={view.running}
            onClick={() =>
              void onChooseFile().then((f) => {
                if (typeof f === "string") setEngine(f);
              })
            }
          >
            Other engine…
          </button>
        </label>
        <p className="hint">
          Each finished job replaces its game in the database with the analysis
          added (Merge result with original game). Games open in a tab, changed
          since the job was added, or with annotations that are not all decoded
          are skipped.
        </p>
        {progress && (
          <p role="status" className="hint">
            {progress}
          </p>
        )}
        {error && <p role="alert">{error}</p>}
        <div className="mask-buttons">
          {view.running ? (
            <button type="button" onClick={onStop}>
              Stop
            </button>
          ) : (
            <button
              type="button"
              disabled={!engine || !waiting}
              onClick={() => engine && onRun({ engine, options })}
            >
              Start/Continue
            </button>
          )}
          <button type="button" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
