import type { GameNode, SquareMark, ArrowMark } from "../api/types";
import type { DrawnMark } from "../components/Board";

/**
 * A node's marks after drawing `mark` on it, as in the reference app: drawing a mark
 * again in its colour removes it; in another colour, recolours it.
 */
export function toggleMark(
  node: Pick<GameNode, "squares" | "arrows">,
  mark: DrawnMark,
): { squares: SquareMark[]; arrows: ArrowMark[] } {
  const squares = [...(node.squares ?? [])];
  const arrows = [...(node.arrows ?? [])];
  if (mark.from === mark.to) {
    const at = squares.findIndex((s) => s.square === mark.from);
    const same = at >= 0 && squares[at].color === mark.color;
    if (at >= 0) squares.splice(at, 1);
    if (!same) squares.push({ color: mark.color, square: mark.from });
  } else {
    const at = arrows.findIndex(
      (a) => a.from === mark.from && a.to === mark.to,
    );
    const same = at >= 0 && arrows[at].color === mark.color;
    if (at >= 0) arrows.splice(at, 1);
    if (!same) arrows.push({ color: mark.color, from: mark.from, to: mark.to });
  }
  return { squares, arrows };
}
