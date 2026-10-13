import { pieceImages } from "../board/pieces";
import type { ArrowMark, SquareMark } from "../api/types";

/** Mark colour letters as used by PGN [%csl]/[%cal]. */
const markColors: Record<string, string> = {
  G: "green",
  Y: "yellow",
  R: "red",
  B: "blue",
  C: "cyan",
  O: "orange",
};
const roles: Record<string, string> = {
  k: "king",
  q: "queen",
  r: "rook",
  b: "bishop",
  n: "knight",
  p: "pawn",
};
export function Board({
  fen,
  flipped,
  legalUci,
  onSquare,
  selectedSquare,
  compact,
  marks,
}: {
  fen: string;
  flipped: boolean;
  legalUci: string[];
  onSquare?: (square: string) => void;
  selectedSquare?: string;
  compact?: boolean;
  marks?: { squares?: SquareMark[]; arrows?: ArrowMark[] };
}) {
  const marked: Record<string, string> = {};
  for (const m of marks?.squares ?? []) marked[m.square] = markColors[m.color];
  const squares: Record<string, string> = {};
  fen
    .split(" ")[0]
    .split("/")
    .forEach((row, i) => {
      let f = 0;
      for (const p of row) {
        if (/[1-8]/.test(p)) f += Number(p);
        else squares[`${"abcdefgh"[f++]}${8 - i}`] = p;
      }
    });
  const files = flipped ? "hgfedcba" : "abcdefgh";
  const ranks = flipped ? "12345678" : "87654321";
  return (
    <div
      role="grid"
      aria-label="Chess board"
      data-flipped={flipped}
      className={compact ? "board compact" : "board"}
    >
      {[...ranks].map((r) => (
        <div role="row" className="board-rank" key={r}>
          {[...files].map((f) => {
            const square = f + r,
              p = squares[square];
            const label = p
              ? `${p === p.toUpperCase() ? "White" : "Black"} ${roles[p.toLowerCase()]}`
              : "empty";
            return (
              <div
                role="gridcell"
                key={square}
                className="board-cell"
                aria-selected={square === selectedSquare}
              >
                <button
                  aria-label={`${square} ${label}`}
                  aria-pressed={square === selectedSquare}
                  className={`square ${(files.indexOf(f) + ranks.indexOf(r)) % 2 ? "dark" : "light"} ${selectedSquare === square ? "selected" : ""}`}
                  data-mark={marked[square]}
                  onClick={() => onSquare?.(square)}
                  data-legal={legalUci.some(
                    (m) =>
                      m.slice(0, 2) === selectedSquare &&
                      m.slice(2, 4) === square,
                  )}
                >
                  {p && (
                    <img
                      className="piece"
                      src={pieceImages[p]}
                      alt=""
                      draggable={false}
                    />
                  )}
                  {f === files[0] && <small className="rank-coord">{r}</small>}
                  {r === ranks[7] && <small className="file-coord">{f}</small>}
                </button>
              </div>
            );
          })}
        </div>
      ))}
      {!!marks?.arrows?.length && (
        <svg
          className="board-arrows"
          viewBox="0 0 8 8"
          aria-label="Board arrows"
          role="img"
        >
          {marks.arrows.map((a, i) => {
            const at = (s: string) => [
              files.indexOf(s[0]) + 0.5,
              ranks.indexOf(s[1]) + 0.5,
            ];
            const [x1, y1] = at(a.from),
              [x2, y2] = at(a.to);
            const length = Math.hypot(x2 - x1, y2 - y1) || 1;
            // Stop the shaft short of the head.
            const [ux, uy] = [(x2 - x1) / length, (y2 - y1) / length];
            const color = markColors[a.color] ?? "green";
            return (
              <g key={i} className={`arrow ${color}`}>
                <title>{`${a.from}–${a.to}`}</title>
                <line
                  x1={x1}
                  y1={y1}
                  x2={x2 - ux * 0.32}
                  y2={y2 - uy * 0.32}
                  strokeWidth={0.16}
                />
                <polygon
                  points={`${x2},${y2} ${x2 - ux * 0.4 - uy * 0.22},${y2 - uy * 0.4 + ux * 0.22} ${x2 - ux * 0.4 + uy * 0.22},${y2 - uy * 0.4 - ux * 0.22}`}
                />
              </g>
            );
          })}
        </svg>
      )}
    </div>
  );
}
