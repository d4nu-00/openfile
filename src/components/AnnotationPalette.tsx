import type { Edit, GameNode } from "../api/types";
import { nagSymbols } from "./NotationTree";
import { SYMBOL_GROUPS, toggleNag, type SymbolSlot } from "../state/symbols";

export { toggleNag };

const LABELS: Record<SymbolSlot, string> = {
  move: "Move",
  position: "Position",
  prefix: "Prefix",
};

/**
 * The reference app's Annotation Palette (Ctrl+Alt+S): symbols for the current move,
 * stored as PGN NAGs. A move has one symbol of each group; clicking a set
 * symbol removes it.
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
      {(Object.keys(SYMBOL_GROUPS) as SymbolSlot[]).map((slot) => (
        <div
          key={slot}
          className="palette-group"
          role="group"
          aria-label={LABELS[slot]}
        >
          {SYMBOL_GROUPS[slot].map(({ nag, name }) => (
            <button
              key={nag}
              disabled={!usable}
              aria-pressed={!!node?.nags.includes(nag)}
              aria-label={name}
              title={`${name} ($${nag})`}
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
