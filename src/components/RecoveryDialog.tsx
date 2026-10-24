import { useEffect, useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type { RecoveryProgress, RecoveryReport } from "../api/types";

export function RecoveryDialog({
  api,
  initialSource,
  onClose,
  onOpen,
}: {
  api: WorkbenchApi;
  initialSource?: string;
  onClose: () => void;
  onOpen: (path: string) => Promise<void>;
}) {
  const [source, setSource] = useState(initialSource ?? "");
  const [destination, setDestination] = useState("");
  const [busy, setBusy] = useState(false);
  const [jobId, setJobId] = useState<string>();
  const [stopping, setStopping] = useState(false);
  const [progress, setProgress] = useState<RecoveryProgress>();
  const [result, setResult] = useState<RecoveryReport>();
  const [error, setError] = useState<string>();
  const active = useRef<string | undefined>(undefined);
  const live = useRef(true);
  const stopped = useRef(false);
  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
      if (active.current)
        void api.cancelRecovery(active.current).catch(() => {});
    };
  }, [api]);
  const explain = (e: unknown) =>
    (e as { message?: string })?.message ?? String(e);
  async function run() {
    if (busy || !source || !destination) return;
    setBusy(true);
    setStopping(false);
    stopped.current = false;
    setError(undefined);
    setResult(undefined);
    setProgress(undefined);
    try {
      const id = await api.beginRecovery();
      if (!live.current) {
        await api.cancelRecovery(id);
        return;
      }
      active.current = id;
      setJobId(id);
      const r = await api.recoverDatabase(id, source, destination, (p) => {
        if (live.current && active.current === id && p.jobId === id)
          setProgress(p);
      });
      if (live.current && active.current === id) setResult(r);
    } catch (e) {
      if (live.current)
        setError(
          stopped.current && (e as { code?: string })?.code === "staleJob"
            ? "Stopped; no result was published."
            : explain(e),
        );
    } finally {
      const id = active.current;
      active.current = undefined;
      if (id) void api.cancelRecovery(id).catch(() => {});
      if (live.current) {
        setJobId(undefined);
        setBusy(false);
        setStopping(false);
      }
    }
  }
  const choose = async (kind: "source" | "destination") => {
    try {
      const p = await (kind === "source"
        ? api.chooseDatabase()
        : api.chooseRecoveryDestination());
      if (typeof p === "string" && live.current) {
        if (kind === "source") setSource(p);
        else setDestination(p);
        setResult(undefined);
        setError(undefined);
      }
    } catch (e) {
      if (live.current) setError(explain(e));
    }
  };
  const classic = !!source && !source.toLowerCase().endsWith(".2cbh");
  return (
    <div className="modal-backdrop">
      <section
        role="dialog"
        aria-modal="true"
        aria-label="Recover readable games"
        className="modal recovery-dialog"
        onKeyDown={(e) => {
          if (e.key === "Escape" && !busy) onClose();
        }}
      >
        <h2>Recover readable games</h2>
        <p>
          Copy readable games into a new verified 2CBH database. Your source
          files stay unchanged.
        </p>
        <label>
          Damaged database{" "}
          <span className="path">{source || "None chosen"}</span>
        </label>
        <button
          type="button"
          disabled={busy}
          autoFocus
          onClick={() => void choose("source")}
        >
          Choose damaged database…
        </button>
        <label>
          New result folder{" "}
          <span className="path">{destination || "None chosen"}</span>
        </label>
        <button
          type="button"
          disabled={busy}
          onClick={() => void choose("destination")}
        >
          Choose new result folder…
        </button>
        <p className="hint">
          Choose a folder name that does not exist. The verified database and
          full entry report will appear together inside it.
        </p>
        <p className="hint">
          2CBH only, up to 2,000,000 entries. Text entries and unsupported games
          are skipped and listed. Readable annotation bytes and metadata are
          retained, including content OpenFile cannot display. Missing or
          damaged required content causes the whole entry to be skipped.
        </p>
        {classic && (
          <p role="alert">
            Classic CBH recovery is not supported. Choose a damaged .2cbh
            database.
          </p>
        )}
        {busy && (
          <div role="status">
            <p>
              {stopping
                ? "Stopping…"
                : progress
                  ? `${progress.stage === "verifying" ? "Verifying" : "Recovering"}: ${progress.done.toLocaleString()} of ${progress.total.toLocaleString()} entries`
                  : "Preparing recovery…"}
            </p>
            <progress
              aria-label="Recovery progress"
              max={progress?.total || 1}
              value={progress?.done || 0}
            />
          </div>
        )}
        {result?.cancelled && (
          <p role="status">Stopped; no result was published.</p>
        )}
        {result && !result.cancelled && (
          <div className="recovery-result">
            <h3>{result.recovered.toLocaleString()} games recovered</h3>
            <p>
              Verified copy: {result.corrupt.toLocaleString()} corrupt entries
              and {result.unsupported.toLocaleString()} unsupported entries
              skipped; {result.warnings.toLocaleString()} warnings.
            </p>
            <p className="path">{result.path}</p>
            <p>
              Full entry report:{" "}
              <span className="path">{result.reportPath}</span>
            </p>
            <div
              className="recovery-problems"
              tabIndex={0}
              aria-label="Recovery report"
            >
              <ul>
                {result.problems.map((p, i) => (
                  <li key={i}>
                    {p.ordinal ? `Entry ${p.ordinal}: ` : "Files: "}
                    {p.message}
                  </li>
                ))}
              </ul>
            </div>
            {result.problemsTruncated && (
              <p className="hint">
                First 500 issues shown. The full report contains every skipped
                entry.
              </p>
            )}
          </div>
        )}
        {error && <p role="alert">{error}</p>}
        <div className="mask-buttons">
          {busy ? (
            <button
              type="button"
              disabled={!jobId || stopping}
              onClick={() => {
                const id = active.current;
                if (id) {
                  stopped.current = true;
                  setStopping(true);
                  void api.cancelRecovery(id).catch((e) => {
                    setError(explain(e));
                    setStopping(false);
                  });
                }
              }}
            >
              Stop
            </button>
          ) : result?.path && !result.cancelled ? (
            <button
              type="button"
              onClick={() => {
                setBusy(true);
                void onOpen(result.path!)
                  .then(onClose)
                  .catch((e) => {
                    setError(explain(e));
                    setBusy(false);
                  });
              }}
            >
              Open recovered database
            </button>
          ) : (
            <button
              type="button"
              disabled={!source || !destination || classic}
              onClick={() => void run()}
            >
              Recover
            </button>
          )}
          <button type="button" disabled={busy} onClick={onClose}>
            Close
          </button>
        </div>
      </section>
    </div>
  );
}
