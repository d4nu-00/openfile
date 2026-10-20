import {
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
} from "lucide-react";
import { Board, type DrawnMark } from "../components/Board";
import { pieceImages } from "../board/pieces";
import type { GameNode, PositionResponse } from "../api/types";

export type Step = "first" | "previous" | "next" | "last";
export const initialFen =
  "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/** Board with the reference app's replay bar underneath, shared by preview and game tab. */
export function BoardPane({
  title,
  position,
  flipped,
  hasGame,
  navigate,
  selectedSquare,
  onSquare,
  onMove,
  promotion,
  onPromote,
  compact,
  node,
  onMark,
}: {
  title: string;
  position?: PositionResponse;
  flipped: boolean;
  hasGame: boolean;
  navigate: (step: Step) => void;
  selectedSquare?: string;
  onSquare?: (square: string) => void;
  onMove?: (from: string, to: string) => void;
  promotion?: string[];
  onPromote?: (uci?: string) => void;
  compact?: boolean;
  /** Current move; its coloured squares and arrows are drawn. */
  node?: GameNode;
  onMark?: (mark: DrawnMark) => void;
}) {
  const steps: [Step, string, typeof ChevronLeft][] = [
    ["first", "First move", ChevronsLeft],
    ["previous", "Previous move", ChevronLeft],
    ["next", "Next move", ChevronRight],
    ["last", "Last move", ChevronsRight],
  ];
  const promotionSquare = promotion?.[0]?.slice(2, 4);
  const promotionFile = promotionSquare
    ? (flipped ? "hgfedcba" : "abcdefgh").indexOf(promotionSquare[0])
    : 0;
  const promotionAtTop = promotionSquare?.[1] === (flipped ? "1" : "8");
  return (
    <div className={compact ? "board-pane compact" : "board-pane"}>
      <div className="pane-title">
        <h2>{title}</h2>
        <span className="turn" data-turn={position?.turn ?? "white"}>
          {position?.turn ?? "white"} to move
        </span>
      </div>
      <div className="board-holder">
        <Board
          fen={position?.fen ?? initialFen}
          flipped={flipped}
          selectedSquare={selectedSquare}
          onSquare={onSquare}
          onMove={onMove}
          legalUci={position?.legalUci ?? []}
          compact={compact}
          marks={node}
          onMark={onMark}
        />
        {promotion && promotion.length > 0 && onPromote && (
          <section
            role="dialog"
            aria-label="Choose promotion"
            className="promotion"
            onClick={(event) => {
              if (event.target === event.currentTarget) onPromote(undefined);
            }}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                event.stopPropagation();
                onPromote(undefined);
              }
            }}
          >
            <div
              className={`promotion-pieces ${promotionAtTop ? "from-top" : "from-bottom"}`}
              style={{ left: `${(promotionFile + 0.5) * 12.5}%` }}
            >
              {(
                [
                  ["q", "Queen"],
                  ["r", "Rook"],
                  ["b", "Bishop"],
                  ["n", "Knight"],
                ] as const
              ).map(([code, label]) => (
                <button
                  key={code}
                  type="button"
                  aria-label={label}
                  title={label}
                  autoFocus={code === "q"}
                  onClick={() =>
                    onPromote(promotion.find((m) => m.endsWith(code)))
                  }
                >
                  <img
                    src={
                      pieceImages[
                        position?.turn === "black" ? code : code.toUpperCase()
                      ]
                    }
                    alt=""
                  />
                </button>
              ))}
            </div>
          </section>
        )}
      </div>
      <div className="replay-bar" aria-label="Move navigation">
        {steps.map(([step, label, Icon]) => (
          <button
            key={step}
            aria-label={label}
            title={label}
            disabled={!hasGame}
            onClick={() => navigate(step)}
          >
            <Icon size={18} aria-hidden />
          </button>
        ))}
      </div>
    </div>
  );
}
