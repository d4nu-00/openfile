import { useState } from "react";
import { Board } from "./Board";
import { pieceImages } from "../board/pieces";

const INITIAL = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const PIECES = ["K", "Q", "R", "B", "N", "P", "k", "q", "r", "b", "n", "p"];
const NAMES: Record<string, string> = {
  k: "king",
  q: "queen",
  r: "rook",
  b: "bishop",
  n: "knight",
  p: "pawn",
};
const pieceName = (p: string) =>
  `${p === p.toUpperCase() ? "White" : "Black"} ${NAMES[p.toLowerCase()]}`;

interface Setup {
  squares: Record<string, string>;
  turn: "w" | "b";
  castling: string;
  ep: string;
  move: string;
}

/** Reads a FEN into the editor; `undefined` when it is not one. */
function parse(fen: string): Setup | undefined {
  const [placement, turn = "w", castling = "-", ep = "-", , move = "1"] = fen
    .trim()
    .split(/\s+/);
  const rows = placement?.split("/") ?? [];
  if (rows.length !== 8 || !["w", "b"].includes(turn)) return undefined;
  const squares: Record<string, string> = {};
  for (let i = 0; i < 8; i++) {
    let f = 0;
    for (const c of rows[i]) {
      if (/[1-8]/.test(c)) f += Number(c);
      else if (PIECES.includes(c) && f < 8)
        squares[`${"abcdefgh"[f++]}${8 - i}`] = c;
      else return undefined;
    }
    if (f !== 8) return undefined;
  }
  return {
    squares,
    turn: turn as "w" | "b",
    castling: castling === "-" ? "" : castling,
    ep: ep === "-" ? "" : ep[0],
    move,
  };
}

function toFen(s: Setup): string {
  const rows = [];
  for (let r = 8; r >= 1; r--) {
    let row = "";
    let empty = 0;
    for (const f of "abcdefgh") {
      const p = s.squares[`${f}${r}`];
      if (!p) empty++;
      else {
        if (empty) row += empty;
        empty = 0;
        row += p;
      }
    }
    rows.push(empty ? row + empty : row);
  }
  const castling =
    ["K", "Q", "k", "q"].filter((c) => s.castling.includes(c)).join("") || "-";
  const ep = s.ep ? `${s.ep}${s.turn === "w" ? 6 : 3}` : "-";
  const move = /^[1-9][0-9]*$/.test(s.move) ? s.move : "1";
  return `${rows.join("/")} ${s.turn} ${castling} ${ep} 0 ${move}`;
}

/**
 * ChessBase's Position Setup (S): place pieces, then start a new game from
 * the position. The workbench checks the position when the game is made.
 */
export function PositionSetup({
  boardFen,
  error,
  onApply,
  onClose,
}: {
  boardFen?: string;
  error?: string;
  onApply: (fen: string) => void;
  onClose: () => void;
}) {
  const [setup, setSetup] = useState<Setup>(
    () => parse(boardFen ?? INITIAL) ?? parse(INITIAL)!,
  );
  const [tool, setTool] = useState("P");
  const [fenText, setFenText] = useState<string>();
  const [fenError, setFenError] = useState("");
  const fen = toFen(setup);
  const update = (patch: Partial<Setup>) => {
    setSetup((s) => ({ ...s, ...patch }));
    setFenText(undefined);
  };
  function place(square: string) {
    const squares = { ...setup.squares };
    if (tool === "" || squares[square] === tool) delete squares[square];
    else squares[square] = tool;
    update({ squares });
  }
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Position setup"
        className="modal position-setup"
        onSubmit={(e) => {
          e.preventDefault();
          onApply(fen);
        }}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Position setup</h2>
        <div className="setup-body">
          <div className="setup-board">
            <Board fen={fen} flipped={false} legalUci={[]} onSquare={place} />
          </div>
          <div className="setup-side">
            <div className="setup-palette" role="radiogroup" aria-label="Piece">
              {PIECES.map((p) => (
                <button
                  key={p}
                  type="button"
                  role="radio"
                  aria-checked={tool === p}
                  aria-label={pieceName(p)}
                  title={pieceName(p)}
                  onClick={() => setTool(p)}
                >
                  <img src={pieceImages[p]} alt="" />
                </button>
              ))}
              <button
                type="button"
                role="radio"
                aria-checked={tool === ""}
                aria-label="Empty square"
                title="Empty square"
                onClick={() => setTool("")}
              >
                ×
              </button>
            </div>
            <fieldset className="mask-row">
              <legend className="mask-label">To move</legend>
              {(
                [
                  ["w", "White"],
                  ["b", "Black"],
                ] as const
              ).map(([value, label]) => (
                <label key={value} className="mask-check">
                  <input
                    type="radio"
                    name="turn"
                    checked={setup.turn === value}
                    onChange={() => update({ turn: value })}
                  />
                  {label}
                </label>
              ))}
            </fieldset>
            <fieldset className="mask-row">
              <legend className="mask-label">Castling</legend>
              {(
                [
                  ["K", "White O-O"],
                  ["Q", "White O-O-O"],
                  ["k", "Black O-O"],
                  ["q", "Black O-O-O"],
                ] as const
              ).map(([flag, label]) => (
                <label key={flag} className="mask-check">
                  <input
                    type="checkbox"
                    checked={setup.castling.includes(flag)}
                    onChange={(e) =>
                      update({
                        castling: e.target.checked
                          ? setup.castling + flag
                          : setup.castling.replace(flag, ""),
                      })
                    }
                  />
                  {label}
                </label>
              ))}
            </fieldset>
            <div className="mask-row">
              <span className="mask-label">En passant</span>
              <select
                aria-label="En passant file"
                value={setup.ep}
                onChange={(e) => update({ ep: e.target.value })}
              >
                <option value="">–</option>
                {[..."abcdefgh"].map((f) => (
                  <option key={f} value={f}>
                    {f}
                  </option>
                ))}
              </select>
              <span className="mask-label">Move number</span>
              <input
                aria-label="Move number"
                inputMode="numeric"
                value={setup.move}
                onChange={(e) => update({ move: e.target.value })}
              />
            </div>
            <div className="mask-row">
              <button type="button" onClick={() => update(parse(INITIAL)!)}>
                Initial position
              </button>
              <button type="button" onClick={() => update({ squares: {} })}>
                Clear board
              </button>
              {boardFen && (
                <button
                  type="button"
                  onClick={() => update(parse(boardFen) ?? setup)}
                >
                  Current board
                </button>
              )}
            </div>
          </div>
        </div>
        <div className="mask-row">
          <span className="mask-label">FEN</span>
          <input
            aria-label="FEN"
            className="mask-fen"
            value={fenText ?? fen}
            onChange={(e) => {
              setFenText(e.target.value);
              const parsed = parse(e.target.value);
              setFenError(parsed ? "" : "Not a FEN position");
              if (parsed) setSetup(parsed);
            }}
          />
        </div>
        {(fenError || error) && <p role="alert">{fenError || error}</p>}
        <div className="mask-buttons">
          <button type="submit">OK</button>
          <button type="button" onClick={onClose}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}
