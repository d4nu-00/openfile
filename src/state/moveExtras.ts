import type { GameView, GameNode, SquareMark, ArrowMark } from "../api/types";
import { MEDAL_NAMES } from "../api/types";

/** The names of the medal bits that are set. */
export function medalNames(bits: number): string[] {
  return MEDAL_NAMES.filter((_, i) => (bits >>> i) & 1);
}

/** "0:00:44" → "0:44"; hours are kept when there are some. */
export function shortTime(text: string): string {
  const m = /^0:(\d\d):(\d\d)$/.exec(text);
  return m ? `${Number(m[1])}:${m[2]}` : text;
}

/**
 * "h:mm:ss", "m:ss" or "s" as the database formats pack a move time:
 * hours, minutes and seconds in the three high bytes. Undefined when invalid.
 */
export function packMoveTime(text: string): number | undefined {
  const parts = text.trim().split(":");
  if (parts.length > 3 || parts.some((p) => !/^\d+$/.test(p))) return;
  const [h, m, s] = [0, 0, 0, ...parts.map(Number)].slice(-3);
  if (h > 255 || m > 59 || s > 59) return;
  return ((h << 24) | (m << 16) | (s << 8)) >>> 0;
}

/**
 * The board's marks for the selected move: its own squares and arrows, plus
 * every pawn's square for a pawn structure mark and the piece's moves as
 * arrows for a piece path mark (the workbench's drawing).
 */
export function boardMarks(
  game: GameView | undefined,
  nodeId: number,
  fen: string | undefined,
): GameNode | undefined {
  const node = game?.nodes[nodeId];
  const x = node?.extras;
  if (!game || !node || !x || (!x.pawnStructure && !x.piecePath)) return node;
  const squares: SquareMark[] = [...(node.squares ?? [])];
  const arrows: ArrowMark[] = [...(node.arrows ?? [])];
  if (x.pawnStructure && fen) {
    const rows = fen.split(" ")[0].split("/");
    rows.forEach((row, r) => {
      let file = 0;
      for (const ch of row) {
        if (/\d/.test(ch)) file += Number(ch);
        else {
          if (ch === "p" || ch === "P")
            squares.push({
              color: "Y",
              square: `${"abcdefgh"[file]}${8 - r}`,
            });
          file++;
        }
      }
    });
  }
  if (x.piecePath) {
    // Back from the move: each earlier move that ended where the piece
    // stands came from where it stood before.
    let at = x.piecePath;
    let id: number | undefined = nodeId;
    const path: ArrowMark[] = [];
    while (id !== undefined && path.length < 200) {
      const n: GameNode | undefined = game.nodes[id];
      if (!n?.uci) break;
      if (n.uci.slice(2, 4) === at) {
        path.unshift({ color: "Y", from: n.uci.slice(0, 2), to: at });
        at = n.uci.slice(0, 2);
      }
      id = n.parent;
    }
    arrows.push(...path);
  }
  return { ...node, squares, arrows };
}
