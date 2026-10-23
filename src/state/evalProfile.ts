import { useEffect, useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type {
  AppSettings,
  GameView,
  InstantProgress,
  ProfilePoint,
} from "../api/types";
import { defaultInstant } from "../api/types";

/** The scale's edge, in centipawns (the workbench's choice: ±5 pawns). */
export const PROFILE_EDGE = 500;
/** A move losing this much for its side is critical (the workbench's rule). */
export const CRITICAL_LOSS = 100;
/** Scores are clamped to ±10 pawns, and mates count as the clamp. */
const CLAMP = 1000;

/** Main-line node ids by ply; entry 0 is the start (node 0). */
export function mainLine(game?: GameView): number[] {
  if (!game) return [];
  const line = [0];
  let at = game.nodes[0];
  while (at?.children.length) {
    const next = at.children[0];
    line.push(next);
    at = game.nodes[next];
  }
  return line;
}

/** A point's value from White's side, clamped; mates at the clamp. */
export function clamped(p: ProfilePoint): number {
  if (p.mate != null)
    return p.mate > 0 || (p.mate === 0 && p.cp == null) ? CLAMP : -CLAMP;
  return Math.max(-CLAMP, Math.min(CLAMP, p.cp ?? 0));
}

/** A point as text, from White's side: "+0.35", "−M3". */
export function pointText(p: ProfilePoint): string {
  if (p.mate != null) return `${p.mate >= 0 ? "+" : "−"}M${Math.abs(p.mate)}`;
  const v = (p.cp ?? 0) / 100;
  return `${v >= 0 ? "+" : "−"}${Math.abs(v).toFixed(2)}`;
}

/**
 * Plies whose move loses at least CRITICAL_LOSS for the side that played
 * it, from the evaluations before and after it, unless that side still has
 * more than PROFILE_EDGE after it. `whiteFirst` says who moves at ply 1.
 */
export function criticalPlies(
  points: ProfilePoint[],
  whiteFirst: boolean,
): number[] {
  const at = new Map(points.map((p) => [p.ply, clamped(p)]));
  const out: number[] = [];
  for (const p of points) {
    if (p.ply === 0) continue;
    const before = at.get(p.ply - 1);
    if (before === undefined) continue;
    const whiteMoved = (p.ply % 2 === 1) === whiteFirst;
    const sign = whiteMoved ? 1 : -1;
    const loss = (before - clamped(p)) * sign;
    if (loss >= CRITICAL_LOSS && clamped(p) * sign <= PROFILE_EDGE)
      out.push(p.ply);
  }
  return out;
}

/** What instant analysis has for a document's main line. */
export interface InstantState {
  documentId: string;
  /** The main line it analysed, as its moves. */
  line: string;
  progress?: InstantProgress;
  running: boolean;
  error?: string;
}

/**
 * The reference app's Instant Analysis: while it is on, the shown game's
 * main line is analysed in the backend with the default engine, and again
 * whenever the game or its main line changes. One runs at a time.
 */
export function useInstantAnalysis(
  api: WorkbenchApi,
  settings: AppSettings,
  documentId: string | undefined,
  game: GameView | undefined,
) {
  const options = settings.instantAnalysis ?? defaultInstant;
  const engine = settings.engines.find((e) => e.id === settings.defaultEngine);
  const ids = mainLine(game);
  const line = ids
    .slice(1)
    .map((id) => game?.nodes[id]?.uci ?? "")
    .join(" ");
  const [state, setState] = useState<InstantState>();
  /** Bumped by Restart, so the same line runs again. */
  const [generation, setGeneration] = useState(0);
  /** The line Stop was pressed for: not restarted until it changes. */
  const stopped = useRef<string | undefined>(undefined);
  const ticket = useRef(0);
  const key = documentId ? `${documentId}|${line}` : undefined;

  useEffect(() => {
    if (!options.enabled || !documentId || !key || stopped.current === key)
      return;
    if (!engine) {
      setState({
        documentId,
        line,
        running: false,
        error:
          "Choose a default engine in Engine Management for Instant Analysis",
      });
      return;
    }
    const mine = ++ticket.current;
    // Stepping through games quickly starts only the last one.
    const timer = setTimeout(() => {
      setState({ documentId, line, running: true });
      api
        .instantAnalysis(
          documentId,
          {
            engine: engine.path,
            options: engine.options,
            level: options.level,
            endless: options.endless,
          },
          (progress) => {
            if (ticket.current === mine)
              setState({ documentId, line, running: true, progress });
          },
        )
        .then((progress) => {
          if (ticket.current === mine)
            setState({ documentId, line, running: false, progress });
        })
        .catch((e: unknown) => {
          if (ticket.current === mine)
            setState({
              documentId,
              line,
              running: false,
              error: (e as { message?: string })?.message ?? String(e),
            });
        });
    }, 300);
    return () => {
      clearTimeout(timer);
      if (ticket.current === mine) {
        ticket.current++;
        void api.cancelInstantAnalysis().catch(() => {});
      }
    };
  }, [
    key,
    options.enabled,
    options.level,
    options.endless,
    engine?.path,
    api,
    generation,
  ]);

  const current =
    state && state.documentId === documentId && state.line === line
      ? state
      : undefined;
  return {
    state: current,
    stop() {
      stopped.current = key;
      ticket.current++;
      void api.cancelInstantAnalysis().catch(() => {});
      setState((s) => s && { ...s, running: false });
    },
    /** Starts again after Stop, for the same line. */
    restart() {
      stopped.current = undefined;
      setState(undefined);
      setGeneration((g) => g + 1);
    },
  };
}
