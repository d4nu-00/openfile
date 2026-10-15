import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import type { GameNode } from "../api/types";

export const nagSymbols: Record<number, string> = {
  1: "!",
  2: "?",
  3: "!!",
  4: "??",
  5: "!?",
  6: "?!",
  7: "□",
  8: "□",
  10: "=",
  11: "=",
  13: "∞",
  14: "⩲",
  15: "⩱",
  16: "±",
  17: "∓",
  18: "+−",
  19: "−+",
  22: "⨀",
  32: "⟳",
  36: "↑",
  40: "→",
  44: "=∞",
  132: "⇆",
  138: "⨁",
  140: "∆",
  141: "∇",
  142: "⌓",
  143: "≤",
  144: "=",
  145: "RR",
  146: "N",
};
const nagText = (n: number) => nagSymbols[n] ?? `$${n}`;

type Token =
  | { kind: "move"; node: GameNode; depth: number; number?: string }
  /** The variations of a folded branch, shown as one button. */
  | { kind: "fold"; node: GameNode; depth: number; count: number }
  | { kind: "open"; depth: number }
  | { kind: "close"; depth: number };

/** Linearise a move tree in PGN order: main reply, sibling variations, continuation. */
export function notationTokens(
  nodes: GameNode[],
  startFen?: string,
  folded?: ReadonlySet<number>,
): Token[] {
  const fields = (startFen ?? "").split(" ");
  const basePly =
    (Math.max(1, Number(fields[5]) || 1) - 1) * 2 + (fields[1] === "b" ? 1 : 0);
  const ply = new Map<number, number>([[0, basePly]]);
  type Frame =
    | { k: "emit"; id: number; depth: number; force: boolean }
    | { k: "after"; id: number; depth: number }
    | { k: "open"; depth: number }
    | { k: "close"; depth: number }
    | { k: "fold"; id: number; depth: number };
  const out: Token[] = [];
  const seen = new Set<number>();
  const stack: Frame[] = [{ k: "after", id: 0, depth: 0 }];
  let forceNext = true;
  while (stack.length) {
    const f = stack.pop()!;
    if (f.k === "open" || f.k === "close") {
      out.push(
        f.k === "open"
          ? { kind: "open", depth: f.depth }
          : { kind: "close", depth: f.depth },
      );
      forceNext = true;
      continue;
    }
    const node = nodes[f.id];
    if (!node) continue;
    if (f.k === "fold") {
      out.push({
        kind: "fold",
        node,
        depth: f.depth,
        count: node.children.length - 1,
      });
      forceNext = true;
      continue;
    }
    if (f.k === "emit") {
      if (seen.has(f.id)) break;
      seen.add(f.id);
      const p = ply.get(node.parent ?? 0) ?? basePly;
      ply.set(f.id, p + 1);
      const white = p % 2 === 0;
      const n = Math.floor(p / 2) + 1;
      const before = (node.preComments?.length ?? 0) > 0;
      const number = white
        ? `${n}.`
        : forceNext || f.force || before
          ? `${n}...`
          : undefined;
      out.push({ kind: "move", node, depth: f.depth, number });
      forceNext = node.comments.length > 0;
      continue;
    }
    const kids = node.children;
    if (!kids.length) continue;
    stack.push({ k: "after", id: kids[0], depth: f.depth });
    if (kids.length > 1 && folded?.has(f.id)) {
      stack.push({ k: "fold", id: f.id, depth: f.depth });
      stack.push({ k: "emit", id: kids[0], depth: f.depth, force: false });
      continue;
    }
    for (let i = kids.length - 1; i >= 1; i--) {
      stack.push({ k: "close", depth: f.depth + 1 });
      stack.push({ k: "after", id: kids[i], depth: f.depth + 1 });
      stack.push({ k: "emit", id: kids[i], depth: f.depth + 1, force: true });
      stack.push({ k: "open", depth: f.depth + 1 });
    }
    stack.push({ k: "emit", id: kids[0], depth: f.depth, force: false });
  }
  return out;
}

const WINDOW = 200;

export function NotationTree({
  nodes,
  selection,
  onSelect,
  startFen,
  folded,
  onUnfold,
}: {
  nodes: GameNode[];
  selection: number;
  onSelect: (id: number) => void;
  startFen?: string;
  /** Branch nodes whose variations are folded (ChessBase's Fold). */
  folded?: ReadonlySet<number>;
  onUnfold?: (id: number) => void;
}) {
  const tokens = useMemo(
    () => notationTokens(nodes, startFen, folded),
    [nodes, startFen, folded],
  );
  const moveIndex = useMemo(
    () => tokens.flatMap((t, i) => (t.kind === "move" ? [i] : [])),
    [tokens],
  );
  const [start, setStart] = useState(0);
  const current = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const m = moveIndex.findIndex((i) => {
      const t = tokens[i];
      return t.kind === "move" && t.node.id === selection;
    });
    if (m >= 0 && (m < start || m >= start + WINDOW))
      setStart(Math.max(0, m - 50));
  }, [selection, tokens, moveIndex]);
  useEffect(() => {
    current.current?.scrollIntoView?.({ block: "nearest" });
  }, [selection, start]);

  const from = moveIndex[start] ?? 0;
  const to = moveIndex[start + WINDOW] ?? tokens.length;
  const visible = tokens.slice(from, to);
  // Top-level variations become their own indented paragraphs, as in ChessBase.
  const blocks: { depth: number; items: Token[] }[] = [];
  let block = { depth: 0, items: [] as Token[] };
  for (const t of visible) {
    if (t.kind === "open" && t.depth === 1) {
      if (block.items.length) blocks.push(block);
      block = { depth: 1, items: [] };
      continue;
    }
    if (t.kind === "close" && t.depth === 1) {
      if (block.items.length) blocks.push(block);
      block = { depth: 0, items: [] };
      continue;
    }
    block.items.push(t);
  }
  if (block.items.length) blocks.push(block);

  return (
    <section aria-label="Game notation" className="notation">
      <div className="notation-tools">
        <button
          aria-current={selection === 0 ? "step" : undefined}
          onClick={() => onSelect(0)}
        >
          Starting position
        </button>
        <span>{nodes.length - 1} moves</span>
      </div>
      <div className="notation-text">
        {start === 0 && (nodes[0]?.comments.length ?? 0) > 0 && (
          <p className="game-comment">
            {nodes[0].comments.map((c, ci) => (
              <span key={ci} className="comment">
                {c}
              </span>
            ))}
          </p>
        )}
        {blocks.map((b, bi) => (
          <p
            key={bi}
            className={b.depth ? "variation-block" : "mainline-block"}
          >
            {b.items.map((t, i) =>
              t.kind === "open" ? (
                <span key={i} className="paren">
                  (
                </span>
              ) : t.kind === "close" ? (
                <span key={i} className="paren">
                  )
                </span>
              ) : t.kind === "fold" ? (
                <button
                  key={`fold${t.node.id}`}
                  className="fold"
                  aria-label={`Unfold ${t.count} variation${t.count > 1 ? "s" : ""}`}
                  title="Folded variations: click to unfold"
                  onClick={() => onUnfold?.(t.node.id)}
                >
                  [+{t.count}]
                </button>
              ) : (
                <Fragment key={t.node.id}>
                  {t.node.preComments?.map((c, ci) => (
                    <span key={`pre${ci}`} className="comment before">
                      {c}
                    </span>
                  ))}
                  {t.number && <span className="move-number">{t.number}</span>}
                  <button
                    ref={t.node.id === selection ? current : undefined}
                    className={t.depth ? "move var" : "move main"}
                    aria-current={t.node.id === selection ? "step" : undefined}
                    onClick={() => onSelect(t.node.id)}
                  >
                    {t.node.san ?? t.node.uci}
                  </button>
                  {t.node.nags.length > 0 && (
                    <span className="nag">
                      {t.node.nags.map(nagText).join("")}
                    </span>
                  )}
                  {t.node.comments.map((c, ci) => (
                    <span key={ci} className="comment">
                      {c}
                    </span>
                  ))}
                </Fragment>
              ),
            )}
          </p>
        ))}
      </div>
      {moveIndex.length > WINDOW && (
        <div className="notation-tools">
          <button
            disabled={start === 0}
            onClick={() => setStart(Math.max(0, start - WINDOW))}
          >
            Earlier notation
          </button>
          <button
            disabled={start + WINDOW >= moveIndex.length}
            onClick={() => setStart(start + WINDOW)}
          >
            Later notation
          </button>
        </div>
      )}
    </section>
  );
}
