import { pieceImages } from "../board/pieces";
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
}: {
  fen: string;
  flipped: boolean;
  legalUci: string[];
  onSquare?: (square: string) => void;
  selectedSquare?: string;
  compact?: boolean;
}) {
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
    </div>
  );
}
