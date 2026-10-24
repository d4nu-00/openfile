import { ChevronLeft, ChevronRight } from "lucide-react";
import type { ProfilePoint } from "../api/types";
import {
  PROFILE_EDGE,
  clamped,
  criticalPlies,
  pointText,
} from "../state/evalProfile";

const W = 1000;
const H = 100;

/** "12.Nf3" / "12...Nf6" for the move reaching `ply`. */
export function plyName(ply: number, san: string, whiteFirst: boolean) {
  const offset = whiteFirst ? 0 : 1;
  const n = Math.floor((ply - 1 + offset) / 2) + 1;
  const white = (ply % 2 === 1) === whiteFirst;
  return `${n}${white ? "." : "..."}${san}`;
}

/**
 * The evaluation profile: the main line's scores from White's side, the
 * current move, and the critical moves. Clicking selects a move.
 */
export function EvalProfile({
  points,
  plies,
  current,
  san,
  whiteFirst,
  compact,
  onSelect,
  status,
}: {
  points: ProfilePoint[];
  /** Plies of the main line. */
  plies: number;
  /** The selected move's ply, when it is on the main line. */
  current?: number;
  /** SAN of the main-line move reaching a ply. */
  san: (ply: number) => string;
  whiteFirst: boolean;
  compact?: boolean;
  onSelect: (ply: number) => void;
  status?: React.ReactNode;
}) {
  const shown = points.filter((p) => p.ply <= plies);
  const critical = criticalPlies(shown, whiteFirst);
  const span = Math.max(plies, 1);
  const x = (ply: number) => (ply / span) * W;
  const y = (v: number) =>
    H / 2 -
    (Math.max(-PROFILE_EDGE, Math.min(PROFILE_EDGE, v)) / PROFILE_EDGE) *
      (H / 2);
  // Gaps split the curve.
  const segments: ProfilePoint[][] = [];
  for (const p of shown) {
    const last = segments.at(-1);
    if (last && last.at(-1)!.ply === p.ply - 1) last.push(p);
    else segments.push([p]);
  }
  const area = segments
    .filter((s) => s.length > 1)
    .map(
      (s) =>
        `M${x(s[0].ply)},${H / 2} ` +
        s.map((p) => `L${x(p.ply)},${y(clamped(p))}`).join(" ") +
        ` L${x(s.at(-1)!.ply)},${H / 2} Z`,
    );
  const before = critical.filter((p) => current === undefined || p < current);
  const after = critical.filter((p) => current === undefined || p > current);
  const at = shown.find((p) => p.ply === current);
  return (
    <div
      className={`eval-profile${compact ? " compact" : ""}`}
      aria-label="Evaluation profile"
    >
      <div className="eval-profile-bar">
        <button
          type="button"
          aria-label="Previous critical move"
          disabled={!before.length}
          onClick={() => onSelect(before.at(-1)!)}
        >
          <ChevronLeft size={12} aria-hidden />
        </button>
        <button
          type="button"
          aria-label="Next critical move"
          disabled={!after.length}
          onClick={() => onSelect(after[0])}
        >
          <ChevronRight size={12} aria-hidden />
        </button>
        <span>
          {critical.length} critical move{critical.length === 1 ? "" : "s"}
        </span>
        {at && current !== undefined && (
          <span>
            {current ? plyName(current, san(current), whiteFirst) : "Start"}:{" "}
            {pointText(at)}
          </span>
        )}
        {status}
      </div>
      {shown.length ? (
        <div className="eval-profile-plot">
          <svg
            viewBox={`0 0 ${W} ${H}`}
            preserveAspectRatio="none"
            role="img"
            aria-label={`Evaluation of ${shown.length} positions`}
            onPointerDown={(e) => {
              const r = e.currentTarget.getBoundingClientRect();
              if (r.width <= 0) return;
              const ply = Math.round(((e.clientX - r.left) / r.width) * span);
              onSelect(Math.max(0, Math.min(plies, ply)));
            }}
          >
            <line className="axis" x1={0} x2={W} y1={H / 2} y2={H / 2} />
            {area.map((d, i) => (
              <path key={i} className="curve" d={d} />
            ))}
            {current !== undefined && (
              <line
                className="current"
                x1={x(current)}
                x2={x(current)}
                y1={0}
                y2={H}
              />
            )}
          </svg>
          {critical.map((ply) => (
            <button
              type="button"
              key={ply}
              className="critical"
              style={{ left: `${(ply / span) * 100}%` }}
              aria-label={`Critical move ${plyName(ply, san(ply), whiteFirst)}`}
              title={`${plyName(ply, san(ply), whiteFirst)} ${pointText(shown.find((p) => p.ply === ply)!)}`}
              onClick={() => onSelect(ply)}
            />
          ))}
        </div>
      ) : (
        <p className="eval-profile-empty">No evaluations for this game yet.</p>
      )}
    </div>
  );
}
