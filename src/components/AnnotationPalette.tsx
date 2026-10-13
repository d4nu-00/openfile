import type { Edit, GameNode } from "../api/types";
import { nagSymbols } from "./NotationTree";

/** Symbol groups; a move has at most one move mark and one evaluation. */
const GROUPS: { label: string; nags: number[]; exclusive: boolean }[] = [
  { label: "Move", nags: [1, 2, 3, 4, 5, 6, 7, 22], exclusive: true },
  {
    label: "Position",
    nags: [10, 13, 14, 15, 16, 17, 18, 19],
    exclusive: true,
  },
  {
    label: "Other",
    nags: [32, 36, 40, 44, 132, 140, 142, 146],
    exclusive: false,
  },
];
const NAMES: Record<number, string> = {
  1: "Good move",
  2: "Mistake",
  3: "Brilliant move",
  4: "Blunder",
  5: "Interesting move",
  6: "Dubious move",
  7: "Only move",
  22: "Zugzwang",
  10: "Equal",
  13: "Unclear",
  14: "White is slightly better",
  15: "Black is slightly better",
  16: "White is better",
  17: "Black is better",
  18: "White is winning",
  19: "Black is winning",
  32: "Development advantage",
  36: "Initiative",
  40: "Attack",
  44: "With compensation",
  132: "Counterplay",
  140: "With the idea",
  142: "Better is",
  146: "Novelty",
};

/** The symbols `node` has after toggling `nag`. */
export function toggleNag(nags: number[], nag: number): number[] {
  if (nags.includes(nag)) return nags.filter((n) => n !== nag);
  const group = GROUPS.find((g) => g.nags.includes(nag));
  const kept = group?.exclusive
    ? nags.filter((n) => !group.nags.includes(n))
    : nags;
  return [...kept, nag];
}

/**
 * ChessBase's Annotation Palette (Ctrl+Alt+S): symbols for the current move,
 * stored as PGN NAGs. Clicking a set symbol removes it.
 */
export function AnnotationPalette({
  node,
  disabled,
  edit,
  onClose,
}: {
  node?: GameNode;
  disabled: boolean;
  edit: (e: Edit) => Promise<unknown>;
  onClose: () => void;
}) {
  const usable = !!node && node.id !== 0 && !disabled;
  return (
    <section className="annotation-palette" aria-label="Annotation palette">
      <div className="pane-header">
        <strong>Annotation palette</strong>
        <button className="link-button" onClick={onClose}>
          Close
        </button>
      </div>
      {GROUPS.map((g) => (
        <div
          key={g.label}
          className="palette-group"
          role="group"
          aria-label={g.label}
        >
          {g.nags.map((nag) => (
            <button
              key={nag}
              disabled={!usable}
              aria-pressed={!!node?.nags.includes(nag)}
              aria-label={NAMES[nag]}
              title={`${NAMES[nag]} ($${nag})`}
              onClick={() =>
                node &&
                void edit({
                  kind: "setNags",
                  node: node.id,
                  nags: toggleNag(node.nags, nag),
                })
              }
            >
              {nagSymbols[nag]}
            </button>
          ))}
        </div>
      ))}
      {node && node.nags.length > 0 && (
        <button
          className="link-button"
          disabled={!usable}
          onClick={() =>
            void edit({ kind: "setNags", node: node.id, nags: [] })
          }
        >
          Remove symbols
        </button>
      )}
    </section>
  );
}
