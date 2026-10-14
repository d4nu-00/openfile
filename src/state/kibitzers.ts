import { useEffect, useRef, useState } from "react";
import type {
  AnalysisInfoDto,
  AppSettings,
  EngineEventDto,
  EngineResponse,
  InstalledEngine,
} from "../api/types";
import type { WorkbenchApi } from "../api/client";
import type { WorkbenchState } from "./workbench";
import { AnalysisGate } from "./analysis";
import { SelectionGate } from "./selection";

const explain = (e: unknown) =>
  (e as { message?: string })?.message ?? String(e);

/** Kibitzers running at once (the backend allows as many). */
export const MAX_KIBITZERS = 4;

/** What a kibitzer pane shows. */
export interface KibitzerView {
  key: string;
  engine: EngineResponse;
  /** The installed engine it runs, if it came from Engine Management. */
  installed?: InstalledEngine;
  enabled: boolean;
  busy: boolean;
  status: string;
  error: string;
  lines: AnalysisInfoDto[];
  /** Lines the engine computes (its MultiPV option), 1 if it has none. */
  multipv: number;
}

/**
 * One engine following the board: its own requests, event channel and
 * retained MultiPV lines. Changes are reported through `changed`.
 */
class Kibitzer {
  view: KibitzerView;
  private gate = new AnalysisGate();
  private pending = new Map<number, AnalysisInfoDto>();
  private dirty = false;
  private off?: () => void;
  private root?: { documentId: string; nodeId: number };
  constructor(
    private api: WorkbenchApi,
    key: string,
    engine: EngineResponse,
    installed: InstalledEngine | undefined,
    private changed: () => void,
  ) {
    const multipv = engine.options.find((o) => o.name === "MultiPV");
    this.view = {
      key,
      engine,
      installed,
      enabled: false,
      busy: false,
      status: "Ready",
      error: "",
      lines: [],
      multipv: Number(multipv?.default ?? 1) || 1,
    };
  }
  private set(patch: Partial<KibitzerView>) {
    this.view = { ...this.view, ...patch };
    this.changed();
  }
  /** Moves retained lines to the view; called on a timer. */
  flush() {
    if (!this.dirty) return;
    this.dirty = false;
    this.set({
      lines: [...this.pending.values()].sort((a, b) => a.multipv - b.multipv),
    });
  }
  private drop() {
    this.gate.invalidate();
    this.off?.();
    this.off = undefined;
    this.pending.clear();
    this.dirty = false;
  }
  /** The board shows another position: its old lines no longer apply. */
  follow(documentId: string | undefined, nodeId: number | undefined) {
    const root =
      documentId && nodeId !== undefined ? { documentId, nodeId } : undefined;
    this.root = root;
    this.drop();
    this.set({ lines: [] });
    if (!this.view.enabled) return;
    if (!root) {
      void this.stop();
      return;
    }
    this.run(root);
  }
  private run(root: { documentId: string; nodeId: number }) {
    const engineId = this.view.engine.engineId;
    const request = this.gate.begin(root.documentId, root.nodeId);
    this.set({ status: `Analysing node ${root.nodeId}`, error: "" });
    const onEvent = (event: EngineEventDto) => {
      if (event.engineId !== engineId || !this.gate.accept(event.requestId))
        return;
      const p = event.payload;
      if (p.kind === "info") {
        // Keep the latest of each MultiPV line, at most 128.
        if (p.multipv >= 1 && p.multipv <= 128) {
          const previous = this.pending.get(p.multipv);
          this.pending.set(p.multipv, {
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
          this.dirty = true;
        }
      } else if (p.kind === "error") {
        this.set({ error: p.message });
        if (/exited|unavailable|failed/i.test(p.message)) {
          this.gate.invalidate();
          this.set({ enabled: false, status: "Engine unavailable" });
        }
      } else if (p.kind === "bestMove")
        this.set({ status: `Search complete: ${p.best}` });
      else this.set({ status: p.message });
    };
    void this.api
      .startAnalysis(engineId, root.documentId, root.nodeId, request, onEvent)
      .then((off) => {
        if (typeof off !== "function") return;
        if (this.gate.accept(request)) this.off = off;
        else off();
      })
      .catch((e) => {
        if (!this.gate.accept(request)) return;
        this.gate.invalidate();
        this.set({
          error: explain(e),
          enabled: false,
          status: "Engine unavailable",
        });
      });
  }
  start() {
    if (this.view.enabled) return;
    this.set({ enabled: true });
    if (this.root) this.run(this.root);
  }
  async stop() {
    this.drop();
    this.set({ enabled: false, status: "Stopped" });
    try {
      await this.api.stopAnalysis(this.view.engine.engineId);
    } catch (e) {
      this.set({ error: explain(e) });
    }
  }
  /** Sets an option; a running analysis is stopped first and restarted. */
  async option(name: string, value: string) {
    const running = this.view.enabled;
    this.set({ busy: true });
    try {
      if (running) await this.stop();
      await this.api.setEngineOption(this.view.engine.engineId, name, value);
      this.set({ error: "", status: `Applied ${name}` });
      if (name === "MultiPV") this.set({ multipv: Number(value) });
      if (running) this.start();
      return true;
    } catch (e) {
      this.set({ error: explain(e) });
      return false;
    } finally {
      this.set({ busy: false });
    }
  }
  /** Increase or Decrease Number of Lines: the MultiPV option. */
  async lines(delta: 1 | -1) {
    const option = this.view.engine.options.find((o) => o.name === "MultiPV");
    if (!option) {
      this.set({ error: "This engine computes one line only (no MultiPV)" });
      return false;
    }
    const max = Math.min(option.max ?? 128, 128);
    const next = Math.max(
      option.min ?? 1,
      Math.min(max, this.view.multipv + delta),
    );
    if (next === this.view.multipv) return false;
    return this.option("MultiPV", String(next));
  }
  async close() {
    this.drop();
    this.set({ enabled: false, busy: true });
    await this.api.closeEngine(this.view.engine.engineId);
  }
}

/** The kibitzers (engine panes) and Engine Management's installed engines. */
export function useKibitzers(
  api: WorkbenchApi,
  state: WorkbenchState,
  settings: AppSettings,
  saveSettings: (s: AppSettings) => Promise<void>,
) {
  const list = useRef<Kibitzer[]>([]);
  const [views, setViews] = useState<KibitzerView[]>([]);
  /** The pane commands act on: the one last added or used. */
  const [active, setActive] = useState<string>();
  const [error, setError] = useState("");
  const [adding, setAdding] = useState(false);
  const picker = useRef(new SelectionGate()).current;
  const keys = useRef(0);
  const refresh = () => setViews(list.current.map((k) => k.view));
  const root = useRef<{ documentId?: string; nodeId?: number }>({});
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  /** Starts `path` as a new kibitzer, analysing at once. */
  async function add(path: string, installed?: InstalledEngine) {
    if (list.current.length >= MAX_KIBITZERS) {
      setError(`At most ${MAX_KIBITZERS} kibitzers run at once`);
      return;
    }
    const ticket = picker.begin();
    setAdding(true);
    try {
      const engine = await api.chooseEngine(path);
      if (!picker.accept(ticket)) {
        void api.closeEngine(engine.engineId).catch(() => {});
        return;
      }
      const k = new Kibitzer(
        api,
        `k-${++keys.current}`,
        engine,
        installed,
        refresh,
      );
      list.current = [...list.current, k];
      setError("");
      for (const [name, value] of Object.entries(installed?.options ?? {}))
        if (engine.options.some((o) => o.name === name))
          await k.option(name, value);
      setActive(k.view.key);
      k.follow(root.current.documentId, root.current.nodeId);
      k.start();
      refresh();
    } catch (e) {
      if (picker.accept(ticket)) setError(explain(e));
    } finally {
      if (picker.accept(ticket)) setAdding(false);
    }
  }
  /** Add Kibitzer from a file chosen by the user. */
  async function choose() {
    const path = await api.pickEngine();
    if (typeof path === "string") await add(path);
  }
  const find = (key?: string) =>
    list.current.find((k) => k.view.key === (key ?? active)) ??
    list.current.at(-1);
  async function remove(key?: string) {
    const k = find(key);
    if (!k) return;
    list.current = list.current.filter((x) => x !== k);
    refresh();
    setActive(list.current.at(-1)?.view.key);
    try {
      await k.close();
    } catch (e) {
      setError(explain(e));
    }
  }
  async function removeAll() {
    for (const k of [...list.current]) await remove(k.view.key);
  }
  /** Default Kibitzer (Alt+F2): starts the default engine, or removes it. */
  async function toggleDefault() {
    const s = settingsRef.current;
    const engine = s.engines.find((e) => e.id === s.defaultEngine);
    if (!engine) {
      setError("Choose a default engine in Engine Management first");
      return false;
    }
    const running = list.current.find(
      (k) => k.view.installed?.id === engine.id,
    );
    if (running) await remove(running.view.key);
    else await add(engine.path, engine);
    return true;
  }
  /** Sets an option, and keeps it for an installed engine. */
  async function option(key: string, name: string, value: string) {
    const k = find(key);
    if (!k || !(await k.option(name, value))) return;
    const id = k.view.installed?.id;
    const s = settingsRef.current;
    if (id && s.engines.some((e) => e.id === id))
      await saveSettings({
        ...s,
        engines: s.engines.map((e) =>
          e.id === id ? { ...e, options: { ...e.options, [name]: value } } : e,
        ),
      }).catch((e) => setError(explain(e)));
  }

  // Every kibitzer follows the game shown.
  const documentId = state.document?.documentId,
    nodeId = state.position?.nodeId;
  useEffect(() => {
    root.current = { documentId, nodeId };
    for (const k of list.current) k.follow(documentId, nodeId);
  }, [documentId, nodeId, state.position?.fen]);
  useEffect(() => {
    const timer = setInterval(() => {
      for (const k of list.current) k.flush();
    }, 125);
    return () => clearInterval(timer);
  }, []);
  useEffect(
    () => () => {
      picker.begin();
      for (const k of list.current) void k.close().catch(() => {});
      list.current = [];
    },
    [api],
  );
  const current = views.find((v) => v.key === active) ?? views.at(-1);
  return {
    kibitzers: views,
    /** The pane commands act on. */
    current,
    setActive,
    error,
    adding,
    add,
    choose,
    remove,
    removeAll,
    toggleDefault,
    option,
    start: (key?: string) => find(key)?.start(),
    stop: (key?: string) => find(key)?.stop(),
    stopAll: () => Promise.all(list.current.map((k) => k.stop())),
    lines: (delta: 1 | -1, key?: string) => find(key)?.lines(delta),
  };
}
