import { useEffect, useMemo, useRef, useState } from "react";
import type { GameNode } from "../api/types";
export function NotationTree({
  nodes,
  selection,
  onSelect,
}: {
  nodes: GameNode[];
  selection: number;
  onSelect: (id: number) => void;
}) {
  const rows = useMemo(() => {
    const out: { node: GameNode; depth: number }[] = [];
    const stack = (nodes[0] ? [...nodes[0].children].reverse() : []).map(
      (id) => ({ id, depth: 0 }),
    );
    const seen = new Set<number>();
    while (stack.length) {
      const { id, depth } = stack.pop()!;
      if (seen.has(id)) break;
      seen.add(id);
      const node = nodes[id];
      if (!node) continue;
      out.push({ node, depth });
      [...node.children]
        .reverse()
        .forEach((child, i) =>
          stack.push({
            id: child,
            depth: depth + (i < node.children.length - 1 ? 1 : 0),
          }),
        );
    }
    return out;
  }, [nodes]);
  const [start, setStart] = useState(0);
  const selected = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const i = rows.findIndex((r) => r.node.id === selection);
    if (i >= 0) setStart(Math.max(0, i - 50));
  }, [selection, rows]);
  useEffect(() => {
    selected.current?.scrollIntoView?.({ block: "nearest" });
  }, [selection, start]);
  return (
    <section aria-label="Game notation" className="notation">
      <div className="notation-tools">
        <button onClick={() => onSelect(0)}>Starting position</button>
        <span>{nodes.length - 1} moves</span>
      </div>
      {rows.slice(start, start + 200).map(({ node, depth }) => (
        <div
          key={node.id}
          className="move-row"
          style={{ paddingLeft: Math.min(depth, 8) * 14 }}
        >
          <button
            ref={node.id === selection ? selected : undefined}
            aria-current={node.id === selection ? "step" : undefined}
            onClick={() => onSelect(node.id)}
          >
            {node.san ?? node.uci}
          </button>
          <span className="nag">{node.nags.map((n) => `$${n}`).join(" ")}</span>
          {node.comments.map((c, i) => (
            <span key={i} className="comment">
              {c}
            </span>
          ))}
        </div>
      ))}
      <div className="notation-tools">
        <button
          disabled={start === 0}
          onClick={() => setStart(Math.max(0, start - 200))}
        >
          Earlier notation
        </button>
        <button
          disabled={start + 200 >= rows.length}
          onClick={() => setStart(start + 200)}
        >
          Later notation
        </button>
      </div>
    </section>
  );
}
