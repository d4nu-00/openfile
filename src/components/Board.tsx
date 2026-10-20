import { useEffect, useRef, useState, type PointerEvent } from "react";
import { pieceImages } from "../board/pieces";
import type { ArrowMark, SquareMark } from "../api/types";

/** A square (`from` = `to`) or an arrow drawn with the mouse. */
export interface DrawnMark {
  from: string;
  to: string;
  color: string;
}
/**
 * The reference app's colours for graphical commentary: green, Shift for red and
 * Ctrl for yellow. The right button or Alt with the left button draws.
 */
export function drawColor(e: { shiftKey: boolean; ctrlKey: boolean }) {
  return e.shiftKey ? "R" : e.ctrlKey ? "Y" : "G";
}

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
  onMove,
  onRelocate,
  selectedSquare,
  compact,
  marks,
  onMark,
}: {
  fen: string;
  flipped: boolean;
  legalUci: string[];
  onSquare?: (square: string) => void;
  /** A legal move dropped from a piece onto its destination. */
  onMove?: (from: string, to: string) => void;
  /** Position setup: move a placed piece without consulting game legality. */
  onRelocate?: (from: string, to: string) => void;
  selectedSquare?: string;
  compact?: boolean;
  marks?: { squares?: SquareMark[]; arrows?: ArrowMark[] };
  /** A square or arrow drawn with the right button or Alt+left. */
  onMark?: (mark: DrawnMark) => void;
}) {
  const [drawing, setDrawing] = useState<DrawnMark>();
  const drawingRef = useRef<DrawnMark | undefined>(undefined);
  const [dragging, setDragging] = useState<{
    from: string;
    to?: string;
    piece: string;
    x: number;
    y: number;
  }>();
  const active = useRef<
    | {
        kind: "move" | "mark";
        pointerId: number;
        from: string;
        startX: number;
        startY: number;
        piece?: string;
        dragged: boolean;
      }
    | undefined
  >(undefined);
  /** The left click that ends an Alt+drag selects no square. */
  const swallowClick = useRef(false);
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
  const hintFrom = dragging?.from ?? selectedSquare;
  const canDrop = (from: string, to: string) =>
    !!onRelocate ||
    legalUci.some((m) => m.slice(0, 2) === from && m.slice(2, 4) === to);
  /** The square under the pointer, from the board's own rectangle. */
  const squareAt = (e: PointerEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const col = Math.floor(((e.clientX - r.left) / r.width) * 8);
    const row = Math.floor(((e.clientY - r.top) / r.height) * 8);
    if (!(col >= 0 && col <= 7 && row >= 0 && row <= 7)) return undefined;
    return files[col] + ranks[row];
  };
  const track = (mark?: DrawnMark) => {
    drawingRef.current = mark;
    setDrawing(mark);
  };
  const cancelPointer = () => {
    active.current = undefined;
    swallowClick.current = false;
    track(undefined);
    setDragging(undefined);
  };
  useEffect(() => {
    const escape = (e: KeyboardEvent) => {
      if (e.key === "Escape" && active.current) {
        e.preventDefault();
        active.current = undefined;
        swallowClick.current = false;
        drawingRef.current = undefined;
        setDrawing(undefined);
        setDragging(undefined);
      }
    };
    window.addEventListener("keydown", escape);
    return () => window.removeEventListener("keydown", escape);
  }, []);
  useEffect(() => {
    active.current = undefined;
    drawingRef.current = undefined;
    setDrawing(undefined);
    setDragging(undefined);
  }, [fen, flipped]);
  const handlers = {
    onPointerDown: (e: PointerEvent<HTMLDivElement>) => {
      if (active.current) return;
      const isMark =
        !!onMark && (e.button === 2 || (e.button === 0 && e.altKey));
      if (!isMark && (e.button !== 0 || (!onMove && !onRelocate))) return;
      const from =
        (e.target as HTMLElement).closest<HTMLElement>("[data-square]")?.dataset
          .square ?? squareAt(e);
      if (!from) return;
      const piece = squares[from];
      if (
        !isMark &&
        (!piece ||
          (!onRelocate && !legalUci.some((m) => m.slice(0, 2) === from)))
      )
        return;
      if (isMark) {
        e.preventDefault();
        swallowClick.current = e.button === 0;
        track({ from, to: from, color: drawColor(e) });
      }
      active.current = {
        kind: isMark ? "mark" : "move",
        pointerId: e.pointerId,
        from,
        startX: e.clientX,
        startY: e.clientY,
        piece,
        dragged: false,
      };
      if (isMark) e.currentTarget.setPointerCapture?.(e.pointerId);
    },
    onPointerMove: (e: PointerEvent<HTMLDivElement>) => {
      const current = active.current;
      if (!current || current.pointerId !== e.pointerId) return;
      const to = squareAt(e);
      if (current.kind === "mark") {
        const d = drawingRef.current;
        if (d && to && to !== d.to) track({ ...d, to });
      } else {
        if (
          !current.dragged &&
          Math.hypot(e.clientX - current.startX, e.clientY - current.startY) < 5
        )
          return;
        if (!current.dragged) e.currentTarget.setPointerCapture?.(e.pointerId);
        current.dragged = true;
        e.preventDefault();
        swallowClick.current = true;
        const rect = e.currentTarget.getBoundingClientRect();
        setDragging({
          from: current.from,
          to,
          piece: current.piece!,
          x: e.clientX - rect.left,
          y: e.clientY - rect.top,
        });
      }
    },
    onPointerUp: (e: PointerEvent<HTMLDivElement>) => {
      const current = active.current;
      if (!current || current.pointerId !== e.pointerId) return;
      active.current = undefined;
      if (current.kind === "mark") {
        const d = drawingRef.current;
        track(undefined);
        if (d) onMark?.(d);
      } else {
        const to = squareAt(e);
        setDragging(undefined);
        if (
          current.dragged &&
          to &&
          to !== current.from &&
          canDrop(current.from, to)
        ) {
          if (onRelocate) onRelocate(current.from, to);
          else onMove?.(current.from, to);
        }
      }
      if (swallowClick.current)
        window.setTimeout(() => {
          swallowClick.current = false;
        }, 0);
    },
    onPointerCancel: (e: PointerEvent<HTMLDivElement>) => {
      if (active.current?.pointerId === e.pointerId) cancelPointer();
    },
    onLostPointerCapture: (e: PointerEvent<HTMLDivElement>) => {
      if (active.current?.pointerId === e.pointerId) cancelPointer();
    },
    onContextMenu: (e: { preventDefault: () => void }) => {
      if (onMark) e.preventDefault();
    },
  };
  const arrows = [
    ...(marks?.arrows ?? []),
    ...(drawing && drawing.from !== drawing.to ? [drawing] : []),
  ];
  return (
    <div
      role="grid"
      aria-label="Chess board"
      data-flipped={flipped}
      data-dragging={dragging?.from}
      className={`${compact ? "board compact" : "board"}${onMove || onRelocate || onMark ? " interactive" : ""}`}
      {...handlers}
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
                  type="button"
                  aria-label={`${square} ${label}`}
                  aria-pressed={square === selectedSquare}
                  className={`square ${(files.indexOf(f) + ranks.indexOf(r)) % 2 ? "dark" : "light"} ${selectedSquare === square ? "selected" : ""} ${dragging?.from === square ? "drag-origin" : ""}`}
                  data-drop-target={
                    dragging?.to === square &&
                    square !== dragging.from &&
                    canDrop(dragging.from, square)
                  }
                  data-occupied={!!p}
                  data-mark={
                    drawing?.from === square && drawing.to === square
                      ? markColors[drawing.color]
                      : marked[square]
                  }
                  data-square={square}
                  onClick={() => {
                    if (swallowClick.current) {
                      swallowClick.current = false;
                      return;
                    }
                    onSquare?.(square);
                  }}
                  data-legal={legalUci.some(
                    (m) =>
                      m.slice(0, 2) === hintFrom && m.slice(2, 4) === square,
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
      {dragging && (
        <img
          className="drag-piece"
          src={pieceImages[dragging.piece]}
          alt=""
          draggable={false}
          style={{ left: dragging.x, top: dragging.y }}
        />
      )}
      {!!arrows.length && (
        <svg
          className="board-arrows"
          viewBox="0 0 8 8"
          aria-label="Board arrows"
          role="img"
        >
          {arrows.map((a, i) => {
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
              <g
                key={i}
                className={`arrow ${color}${a === drawing ? " drawing" : ""}`}
              >
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
