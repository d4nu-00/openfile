import { formatScore, useAnalysis } from "../state/analysis";
import type { WorkbenchApi } from "../api/client";
import type { WorkbenchState } from "../state/workbench";
import { EngineOptions } from "./EngineOptions";
export function EnginePanel({
  api,
  state,
}: {
  api: WorkbenchApi;
  state: WorkbenchState;
}) {
  const a = useAnalysis(api, state);
  return (
    <>
      <div className="engine-controls">
        <h3>Analysis</h3>
        <button onClick={() => void a.choose()}>Choose engine</button>
        {a.engine && (
          <>
            <strong>{a.engine.identity.name ?? "UCI engine"}</strong>
            <button
              disabled={!state.document?.documentId || a.busy}
              onClick={() => (a.enabled ? void a.stop() : a.start())}
            >
              {a.enabled ? "Stop analysis" : "Start analysis"}
            </button>
            <button disabled={a.busy} onClick={() => void a.close()}>
              Close engine
            </button>
          </>
        )}
        <span role="status">{a.status}</span>
      </div>
      {a.error && <p role="alert">{a.error}</p>}
      {a.engine && (
        <EngineOptions
          key={a.engine.engineId}
          options={a.engine.options}
          disabled={a.enabled || a.busy}
          apply={a.option}
        />
      )}
      <p className="score-perspective">Score from White's perspective</p>
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
          {a.lines.map((line) => (
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
                {line.pv.join(" ")}
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
    </>
  );
}
