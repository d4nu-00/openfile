import { useEffect, useRef, useState } from "react";
import type {
  ScoreDto,
  Color,
  EngineResponse,
  AnalysisInfoDto,
  EngineEventDto,
} from "../api/types";
import type { WorkbenchApi } from "../api/client";
import type { WorkbenchState } from "./workbench";
import { SelectionGate } from "./selection";
export function formatScore(
  score: ScoreDto,
  root: Color,
  perspective: Color,
): string {
  const sign = root === perspective ? 1 : -1;
  const value = (score.kind === "cp" ? score.value : score.moves) * sign;
  const bound =
    score.bound === "lowerbound"
      ? sign === 1
        ? "≥"
        : "≤"
      : score.bound === "upperbound"
        ? sign === 1
          ? "≤"
          : "≥"
        : "";
  return (
    bound +
    (score.kind === "cp"
      ? `${value >= 0 ? "+" : ""}${(value / 100).toFixed(2)}`
      : `${value >= 0 ? "+" : "-"}M${Math.abs(value)}`)
  );
}
export class AnalysisGate {
  private gate = new SelectionGate();
  private active?: { requestId: string; documentId: string; nodeId: number };
  begin(documentId: string, nodeId: number) {
    const requestId = this.gate.begin();
    this.active = { requestId, documentId, nodeId };
    return requestId;
  }
  accept(requestId: string) {
    return this.active?.requestId === requestId;
  }
  invalidate() {
    this.gate.begin();
    this.active = undefined;
  }
}
export function useAnalysis(api: WorkbenchApi, state: WorkbenchState) {
  const [engine, setEngine] = useState<EngineResponse>();
  const engineRef = useRef<EngineResponse | undefined>(undefined);
  const [enabled, setEnabled] = useState(false),
    [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("Choose a local UCI engine.");
  const [error, setError] = useState("");
  const [lines, setLines] = useState<AnalysisInfoDto[]>([]);
  const pending = useRef(new Map<number, AnalysisInfoDto>());
  const changed = useRef(false);
  const gate = useRef(new AnalysisGate()).current,
    picker = useRef(new SelectionGate()).current;
  const unsubscribe = useRef<(() => void) | undefined>(undefined);
  const explain = (e: unknown) =>
    (e as { message?: string })?.message ?? String(e);
  async function choose() {
    const ticket = picker.begin();
    try {
      const path = await api.pickEngine();
      if (typeof path !== "string" || !picker.accept(ticket)) return;
      setBusy(true);
      const response = await api.chooseEngine(path);
      if (!picker.accept(ticket)) {
        void api.closeEngine(response.engineId).catch(() => {});
        return;
      }
      gate.invalidate();
      unsubscribe.current?.();
      pending.current.clear();
      setLines([]);
      setEnabled(false);
      const old = engineRef.current;
      engineRef.current = response;
      setEngine(response);
      setStatus("Ready");
      setError("");
      if (old && old.engineId !== response.engineId)
        void api.closeEngine(old.engineId).catch(() => {});
    } catch (e) {
      if (picker.accept(ticket)) setError(explain(e));
    } finally {
      if (picker.accept(ticket)) setBusy(false);
    }
  }
  async function stop() {
    gate.invalidate();
    unsubscribe.current?.();
    pending.current.clear();
    setEnabled(false);
    setStatus("Stopped");
    try {
      if (engineRef.current) await api.stopAnalysis(engineRef.current.engineId);
    } catch (e) {
      setError(explain(e));
    }
  }
  async function close() {
    const old = engineRef.current;
    if (!old) return;
    gate.invalidate();
    unsubscribe.current?.();
    pending.current.clear();
    setEnabled(false);
    setBusy(true);
    try {
      await api.closeEngine(old.engineId);
      if (engineRef.current?.engineId === old.engineId) {
        engineRef.current = undefined;
        setEngine(undefined);
        setLines([]);
        setStatus("Choose a local UCI engine.");
        setError("");
      }
    } catch (e) {
      setError(explain(e));
    } finally {
      setBusy(false);
    }
  }
  async function option(name: string, value: string) {
    if (!engineRef.current) return;
    setBusy(true);
    try {
      await api.setEngineOption(engineRef.current.engineId, name, value);
      setError("");
      setStatus(`Applied ${name}`);
    } catch (e) {
      setError(explain(e));
    } finally {
      setBusy(false);
    }
  }
  useEffect(() => {
    const timer = setInterval(() => {
      if (changed.current) {
        setLines(
          [...pending.current.values()].sort((a, b) => a.multipv - b.multipv),
        );
        changed.current = false;
      }
    }, 125);
    return () => clearInterval(timer);
  }, []);
  const documentId = state.document?.documentId,
    nodeId = state.position?.nodeId;
  useEffect(() => {
    // Retained results belong to this root even when analysis is stopped.
    pending.current.clear();
    changed.current = false;
    setLines([]);
  }, [documentId, nodeId, state.position?.fen]);
  useEffect(() => {
    if (!enabled || !engine) return;
    if (!documentId || nodeId === undefined) {
      void stop();
      return;
    }
    const request = gate.begin(documentId, nodeId);
    unsubscribe.current?.();
    pending.current.clear();
    setLines([]);
    setStatus(`Analysing node ${nodeId}`);
    setError("");
    const onEvent = (event: EngineEventDto) => {
      if (event.engineId !== engine.engineId || !gate.accept(event.requestId))
        return;
      const p = event.payload;
      if (p.kind === "info") {
        // Retain every latest MultiPV line across flushes, bounded to 128 rows.
        if (p.multipv >= 1 && p.multipv <= 128) {
          const previous = pending.current.get(p.multipv);
          pending.current.set(p.multipv, {
            ...previous,
            ...p,
            depth: p.depth ?? previous?.depth,
            nodes: p.nodes ?? previous?.nodes,
            score: p.score ?? previous?.score,
            pv: p.pv ?? previous?.pv,
            rawPv: p.rawPv ?? previous?.rawPv,
            diagnostics:
              p.pv != null
                ? p.diagnostics
                : (previous?.diagnostics ?? p.diagnostics),
          });
          changed.current = true;
        }
      } else if (p.kind === "error") {
        setError(p.message);
        if (/exited|unavailable|failed/i.test(p.message)) {
          setEnabled(false);
          gate.invalidate();
          setStatus("Engine unavailable");
        }
      } else if (p.kind === "bestMove") setStatus(`Search complete: ${p.best}`);
      else setStatus(p.message);
    };
    void api
      .startAnalysis(engine.engineId, documentId, nodeId, request, onEvent)
      .then((off) => {
        if (!gate.accept(request)) {
          if (typeof off === "function") off();
        } else if (typeof off === "function") unsubscribe.current = off;
      })
      .catch((e) => {
        if (gate.accept(request)) {
          setError(explain(e));
          setEnabled(false);
          gate.invalidate();
          setStatus("Engine unavailable");
        }
      });
    return () => {
      gate.invalidate();
      unsubscribe.current?.();
      unsubscribe.current = undefined;
      pending.current.clear();
    };
  }, [api, enabled, engine?.engineId, documentId, nodeId, state.position?.fen]);
  useEffect(
    () => () => {
      picker.begin();
      gate.invalidate();
      unsubscribe.current?.();
      if (engineRef.current)
        void api.closeEngine(engineRef.current.engineId).catch(() => {});
    },
    [api],
  );
  return {
    engine,
    enabled,
    busy,
    status,
    error,
    lines,
    choose,
    stop,
    option,
    close,
    start: () => setEnabled(true),
  };
}
