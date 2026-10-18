import { useEffect, useState } from "react";
import type { PositionQuery } from "../api/types";
import { pieceImages } from "../board/pieces";

type BoardName = "lookFor" | "or" | "exclude";
/** Pieces per square: KQRBNP White, kqrbnp Black, X/x any piece. */
type Squares = Record<string, string[]>;

const PALETTE = ["K", "Q", "R", "B", "N", "P", "X"];
const NAMES: Record<string, string> = {
  k: "king",
  q: "queen",
  r: "rook",
  b: "bishop",
  n: "knight",
  p: "pawn",
  x: "piece (any)",
};
const pieceName = (p: string) =>
  `${p === p.toUpperCase() ? "White" : "Black"} ${NAMES[p.toLowerCase()]}`;
/** At most this many pieces on one square, drawn 2 × 2. */
const PER_SQUARE = 4;
const BOARDS: [BoardName, string][] = [
  ["lookFor", "'Look for' board"],
  ["or", "'Or' board"],
  ["exclude", "'Exclude' board"],
];

/** The squares of a FEN's placement, or undefined. */
function fromFen(fen: string): Squares | undefined {
  const rows = fen.trim().split(/\s+/)[0]?.split("/") ?? [];
  if (rows.length !== 8) return undefined;
  const out: Squares = {};
  for (let i = 0; i < 8; i++) {
    let f = 0;
    for (const c of rows[i]) {
      if (/[1-8]/.test(c)) f += Number(c);
      else if (/[KQRBNPkqrbnp]/.test(c) && f < 8)
        out[`${"abcdefgh"[f++]}${8 - i}`] = [c];
      else return undefined;
    }
    if (f !== 8) return undefined;
  }
  return out;
}

const list = (s: Squares) =>
  Object.entries(s).flatMap(([sq, ps]) =>
    ps.map((p) => [sq, p] as [string, string]),
  );
const toSquares = (pieces: [string, string][] = []) => {
  const out: Squares = {};
  for (const [sq, p] of pieces) (out[sq] ??= []).push(p);
  return out;
};
const num = (v: string) => (v.trim() === "" ? undefined : Number(v));

/** One piece, or "any piece" as a disc in its colour. */
function PieceIcon({ piece }: { piece: string }) {
  if (piece.toLowerCase() === "x")
    return (
      <span
        className={`any-piece ${piece === "X" ? "white" : "black"}`}
        aria-hidden
      />
    );
  return <img src={pieceImages[piece]} alt="" />;
}

/**
 * The search mask's position tab, after the reference app's dialog 314: three
 * boards ('Look for', 'Or', 'Exclude'), mirroring and a move window. With
 * Exact position, the 'Look for' board is searched as a whole position.
 */
export function PositionMask({
  initial,
  boardFen,
  onChange,
}: {
  initial?: PositionQuery;
  /** The current board, for Copy board. */
  boardFen?: string;
  onChange: (query: PositionQuery, valid: boolean) => void;
}) {
  const [boards, setBoards] = useState<Record<BoardName, Squares>>(() => ({
    lookFor: initial
      ? toSquares(initial.lookFor)
      : (fromFen(boardFen ?? "") ?? {}),
    or: toSquares(initial?.or),
    exclude: toSquares(initial?.exclude),
  }));
  const [shown, setShown] = useState<BoardName>("lookFor");
  const [tool, setTool] = useState("N");
  const [options, setOptions] = useState(() => ({
    exact: initial?.exact ?? !!boardFen,
    whiteToMove:
      initial?.whiteToMove ??
      (boardFen ? boardFen.split(/\s+/)[1] !== "b" : undefined),
    horizontal: initial?.horizontal ?? false,
    vertical: initial?.vertical ?? false,
    first: initial?.first,
    last: initial?.last,
    length: initial?.length,
    lines: initial?.lines ?? false,
  }));
  const exact = options.exact;
  const current = boards[exact ? "lookFor" : shown];
  // The parent learns the starting query once.
  useEffect(() => publish(boards, options), []);

  function publish(b: typeof boards, o: typeof options) {
    const query: PositionQuery = {
      lookFor: list(b.lookFor),
      or: o.exact ? [] : list(b.or),
      exclude: o.exact ? [] : list(b.exclude),
      ...o,
      whiteToMove: o.exact ? (o.whiteToMove ?? true) : o.whiteToMove,
    };
    const valid =
      query.lookFor.length + query.or.length + query.exclude.length > 0;
    onChange(query, valid);
  }
  const setOption = (patch: Partial<typeof options>) => {
    const next = { ...options, ...patch };
    setOptions(next);
    publish(boards, next);
  };
  const setBoard = (name: BoardName, squares: Squares) => {
    const next = { ...boards, [name]: squares };
    setBoards(next);
    publish(next, options);
  };

  function place(square: string) {
    const name = exact ? "lookFor" : shown;
    const squares = { ...boards[name] };
    const here = squares[square] ?? [];
    if (tool === "") delete squares[square];
    else if (exact) {
      if (here[0] === tool) delete squares[square];
      else squares[square] = [tool];
    } else if (here.includes(tool)) {
      const rest = here.filter((p) => p !== tool);
      if (rest.length) squares[square] = rest;
      else delete squares[square];
    } else if (here.length < PER_SQUARE) squares[square] = [...here, tool];
    setBoard(name, squares);
  }

  // White's pieces in one row, Black's in the next, as the reference app's palette.
  const palette = [...PALETTE, ...PALETTE.map((p) => p.toLowerCase())].filter(
    (p) => !(exact && p.toLowerCase() === "x"),
  );
  return (
    <div className="position-mask">
      <div className="pattern-board-wrap">
        <div
          className="pattern-board"
          role="grid"
          aria-label={
            exact ? "Position" : BOARDS.find((b) => b[0] === shown)![1]
          }
        >
          {[8, 7, 6, 5, 4, 3, 2, 1].map((rank) =>
            [..."abcdefgh"].map((file, f) => {
              const sq = `${file}${rank}`;
              const pieces = current[sq] ?? [];
              return (
                <button
                  key={sq}
                  type="button"
                  role="gridcell"
                  aria-label={`${sq}${pieces.length ? `: ${pieces.map(pieceName).join(", ")}` : ""}`}
                  className={`square ${(f + rank) % 2 ? "light" : "dark"}${pieces.length > 1 ? " several" : ""}`}
                  onClick={() => place(sq)}
                >
                  {pieces.map((p) => (
                    <PieceIcon key={p} piece={p} />
                  ))}
                </button>
              );
            }),
          )}
        </div>
      </div>
      <div className="setup-side">
        <div
          className="setup-palette pattern-palette"
          role="radiogroup"
          aria-label="Piece"
          style={{ gridTemplateColumns: `repeat(${exact ? 6 : 7}, 34px)` }}
        >
          {palette.map((p) => (
            <button
              key={p}
              type="button"
              role="radio"
              aria-checked={tool === p}
              aria-label={pieceName(p)}
              title={pieceName(p)}
              onClick={() => setTool(p)}
            >
              <PieceIcon piece={p} />
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
        <label className="mask-check">
          <input
            type="checkbox"
            checked={exact}
            onChange={(e) =>
              // Patterns start with either side to move, as the reference app's.
              setOption(
                e.target.checked
                  ? { exact: true }
                  : { exact: false, whiteToMove: undefined },
              )
            }
          />
          Exact position (uses the search booster)
        </label>
        {!exact && (
          <div role="radiogroup" aria-label="Board" className="mask-column">
            {BOARDS.map(([name, label]) => (
              <label key={name} className="mask-check">
                <input
                  type="radio"
                  name="pattern-board"
                  checked={shown === name}
                  onChange={() => setShown(name)}
                />
                {label}
                {Object.keys(boards[name]).length > 0 &&
                  ` (${list(boards[name]).length})`}
              </label>
            ))}
          </div>
        )}
        <fieldset className="mask-row">
          <legend className="mask-label">Mirror</legend>
          <label className="mask-check">
            <input
              type="checkbox"
              checked={options.horizontal}
              onChange={(e) => setOption({ horizontal: e.target.checked })}
            />
            Horizontal
          </label>
          <label className="mask-check">
            <input
              type="checkbox"
              checked={options.vertical}
              onChange={(e) => setOption({ vertical: e.target.checked })}
            />
            Vertical
          </label>
        </fieldset>
        <fieldset className="mask-row">
          <legend className="mask-label">To move</legend>
          {(
            [
              [undefined, "Either"],
              [true, "White"],
              [false, "Black"],
            ] as const
          )
            .filter(([v]) => !(exact && v === undefined))
            .map(([value, label]) => (
              <label key={label} className="mask-check">
                <input
                  type="radio"
                  name="pattern-turn"
                  checked={
                    exact
                      ? (options.whiteToMove ?? true) === value
                      : options.whiteToMove === value
                  }
                  onChange={() => setOption({ whiteToMove: value })}
                />
                {label}
              </label>
            ))}
        </fieldset>
        {!exact && (
          <>
            <div className="mask-row">
              {(
                [
                  ["first", "First"],
                  ["length", "Length"],
                  ["last", "Last"],
                ] as const
              ).map(([key, label]) => (
                <label key={key} className="mask-check">
                  {label}
                  <input
                    aria-label={label}
                    inputMode="numeric"
                    className="mask-number"
                    value={options[key] ?? ""}
                    onChange={(e) => setOption({ [key]: num(e.target.value) })}
                  />
                </label>
              ))}
            </div>
            <label className="mask-check">
              <input
                type="checkbox"
                checked={options.lines}
                onChange={(e) => setOption({ lines: e.target.checked })}
              />
              Include lines in search
            </label>
            <label
              className="mask-check"
              title="The sacrifice test is not available yet"
            >
              <input type="checkbox" disabled />
              Sacrifice
            </label>
          </>
        )}
        <div className="mask-row">
          <button
            type="button"
            disabled={!boardFen}
            onClick={() => {
              const squares = boardFen && fromFen(boardFen);
              if (!squares) return;
              const next = { ...boards, lookFor: squares };
              const o = {
                ...options,
                exact: true,
                whiteToMove: boardFen.split(/\s+/)[1] !== "b",
              };
              setBoards(next);
              setOptions(o);
              setShown("lookFor");
              publish(next, o);
            }}
          >
            Copy board
          </button>
          <button
            type="button"
            onClick={() => setBoard(exact ? "lookFor" : shown, {})}
          >
            Reset
          </button>
        </div>
        <p className="hint">
          {exact
            ? "Finds the board and side to move anywhere in a game, variations included."
            : "Pieces on 'Look for' must stand there; at least one of each kind on 'Or'; none on 'Exclude'. Other squares may hold anything."}
        </p>
      </div>
    </div>
  );
}
