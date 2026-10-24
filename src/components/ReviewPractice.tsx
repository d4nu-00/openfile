import { useEffect, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type { GameView, PositionResponse, ReviewedMove } from "../api/types";
import { Board } from "./Board";
import type { PracticeProgress, PracticeResult } from "../state/reviewPractice";
import { practiceItemKey } from "../state/reviewPractice";

export const criticalMoves = (moves: ReviewedMove[]) =>
  moves.filter(
    (move) =>
      move.bestUci &&
      move.bestLine.length > 0 &&
      ["inaccuracy", "mistake", "missedWin", "losing"].includes(
        move.category ?? "",
      ),
  );

function numbered(move: ReviewedMove, san: string): string {
  return `${move.moveNumber}${move.white ? "." : "..."} ${san}`;
}

function hintFor(san: string): string {
  if (san.startsWith("O-O")) return "Hint: consider castling.";
  const piece: Record<string, string> = {
    K: "king",
    Q: "queen",
    R: "rook",
    B: "bishop",
    N: "knight",
  };
  return `Hint: consider a ${piece[san[0]] ?? "pawn"} move.`;
}

export function ReviewPractice({
  api,
  documentId,
  game,
  moves,
  progress,
  onResult,
}: {
  api: WorkbenchApi;
  documentId: string;
  game: GameView;
  moves: ReviewedMove[];
  progress: PracticeProgress;
  onResult: (move: ReviewedMove, result: PracticeResult) => void;
}) {
  const [index, setIndex] = useState(0);
  const [position, setPosition] = useState<PositionResponse>();
  const [error, setError] = useState<string>();
  const [selected, setSelected] = useState<string>();
  const [promotion, setPromotion] = useState<string[]>([]);
  const [hint, setHint] = useState(false);
  const [revealed, setRevealed] = useState(false);
  const [skipped, setSkipped] = useState(false);
  const [solved, setSolved] = useState(false);
  const [wrong, setWrong] = useState(false);
  const move = moves[index];
  const parent = game.nodes[move.node]?.parent;

  useEffect(() => {
    let live = true;
    setPosition(undefined);
    setError(undefined);
    setSelected(undefined);
    setPromotion([]);
    setHint(false);
    setRevealed(false);
    setSkipped(false);
    setSolved(false);
    setWrong(false);
    if (parent === undefined) {
      setError("This position is no longer available in the game.");
      return;
    }
    void api
      .positionAt(documentId, parent, `practice-${documentId}-${move.node}`)
      .then((p) => {
        if (!live) return;
        if (!p.legalUci.includes(move.bestUci!))
          setError(
            "The suggested move is no longer legal here. Run the review again.",
          );
        else setPosition(p);
      })
      .catch(() => {
        if (live) setError("Could not load this position.");
      });
    return () => {
      live = false;
    };
  }, [api, documentId, move, parent]);

  function tryMove(uci: string) {
    setSelected(undefined);
    setPromotion([]);
    if (uci === move.bestUci) {
      setSolved(true);
      onResult(move, hint || wrong ? "needsPractice" : "solved");
    } else {
      setWrong(true);
      onResult(move, "needsPractice");
    }
  }

  function clickSquare(to: string, from = selected) {
    if (!position || solved || revealed || skipped) return;
    const matches = position.legalUci.filter(
      (uci) => uci.slice(0, 2) === from && uci.slice(2, 4) === to,
    );
    if (matches.length > 1) setPromotion(matches);
    else if (matches.length === 1) tryMove(matches[0]);
    else
      setSelected(
        position.legalUci.some((uci) => uci.slice(0, 2) === to)
          ? to
          : undefined,
      );
  }

  const status = progress[practiceItemKey(move)];
  return (
    <section className="review-practice" aria-label="Learn from mistakes">
      <div className="practice-topline">
        <h3>Learn from mistakes</h3>
        <span>
          {index + 1} of {moves.length}
        </span>
      </div>
      <p className="hint">
        {move.white ? "White" : "Black"} to move. Find a better move than{" "}
        {numbered(move, move.san)}.
      </p>
      <div className="practice-layout">
        <div className="practice-board">
          {position && (
            <Board
              fen={position.fen}
              flipped={!move.white}
              legalUci={position.legalUci}
              selectedSquare={selected}
              onSquare={clickSquare}
              onMove={
                solved || revealed || skipped
                  ? undefined
                  : (from, to) => clickSquare(to, from)
              }
              compact
            />
          )}
          {!position && !error && <p role="status">Loading position…</p>}
          {promotion.length > 0 && (
            <div className="practice-promotion" aria-label="Choose promotion">
              {(["q", "r", "b", "n"] as const).map((piece) => (
                <button
                  key={piece}
                  type="button"
                  onClick={() =>
                    tryMove(promotion.find((uci) => uci.endsWith(piece))!)
                  }
                >
                  {{ q: "Queen", r: "Rook", b: "Bishop", n: "Knight" }[piece]}
                </button>
              ))}
              <button type="button" onClick={() => setPromotion([])}>
                Cancel
              </button>
            </div>
          )}
        </div>
        <div className="practice-side">
          <p className="practice-status" role="status">
            {error ??
              (solved
                ? `Correct: ${numbered(move, move.bestLine[0])}. ${hint || wrong ? "Needs practice" : "Solved without help"}.`
                : revealed
                  ? `Solution: ${numbered(move, move.bestLine.join(" "))}. Needs practice.`
                  : skipped
                    ? "Skipped. Needs practice."
                    : wrong
                      ? "Try another move. This idea needs practice."
                      : status === "solved"
                        ? "Previously solved without help. Try it again."
                        : status === "needsPractice"
                          ? "Needs practice. Try it again."
                          : "Play a move on the board.")}
          </p>
          {hint && !revealed && !solved && !skipped && (
            <p className="hint">{hintFor(move.bestLine[0])}</p>
          )}
          <div className="practice-actions">
            <button
              type="button"
              disabled={!position || solved || revealed || skipped}
              onClick={() => {
                setHint(true);
                onResult(move, "needsPractice");
              }}
            >
              Hint
            </button>
            <button
              type="button"
              disabled={!position || solved || revealed || skipped}
              onClick={() => {
                setRevealed(true);
                onResult(move, "needsPractice");
              }}
            >
              Show solution
            </button>
            <button
              type="button"
              disabled={!position || solved || revealed || skipped}
              onClick={() => {
                setSkipped(true);
                onResult(move, "needsPractice");
              }}
            >
              Skip
            </button>
            {(solved || revealed || skipped) && (
              <button
                type="button"
                onClick={() => {
                  setSelected(undefined);
                  setHint(false);
                  setRevealed(false);
                  setSkipped(false);
                  setSolved(false);
                  setWrong(false);
                }}
              >
                Try again
              </button>
            )}
          </div>
        </div>
      </div>
      <div className="practice-navigation">
        <button
          type="button"
          disabled={index === 0}
          onClick={() => setIndex(index - 1)}
        >
          Previous
        </button>
        <button
          type="button"
          disabled={index === moves.length - 1}
          onClick={() => setIndex(index + 1)}
        >
          Next
        </button>
      </div>
    </section>
  );
}
