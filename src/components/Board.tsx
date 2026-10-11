const symbols: Record<string, string> = {
  K: "♔",
  Q: "♕",
  R: "♖",
  B: "♗",
  N: "♘",
  P: "♙",
  k: "♚",
  q: "♛",
  r: "♜",
  b: "♝",
  n: "♞",
  p: "♟",
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
}: {
  fen: string;
  flipped: boolean;
  legalUci: string[];
  onSquare?: (square: string) => void;
  selectedSquare?: string;
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
      className="board"
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
                  <span
                    className={
                      p === p?.toUpperCase() ? "white-piece" : "black-piece"
                    }
                  >
                    {p ? symbols[p] : ""}
                  </span>
                  <small>{square}</small>
                </button>
              </div>
            );
          })}
        </div>
      ))}
    </div>
  );
}
