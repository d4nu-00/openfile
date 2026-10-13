import { useEffect, useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type { Color, TablebaseOutcome, TablebaseResponse } from "../api/types";
import { SelectionGate } from "../state/selection";

/** A result for `mover` in words, from White's or Black's side. */
export function outcomeText(outcome: TablebaseOutcome, mover: Color) {
  const other = mover === "white" ? "Black" : "White";
  const side = mover === "white" ? "White" : "Black";
  switch (outcome) {
    case "win":
      return `${side} wins`;
    case "maybeWin":
      return `${side} wins unless the 50-move rule intervenes`;
    case "cursedWin":
      return `${side} wins, but it is drawn by the 50-move rule`;
    case "draw":
      return "Draw";
    case "blessedLoss":
      return `${other} wins, but it is drawn by the 50-move rule`;
    case "maybeLoss":
      return `${other} wins unless the 50-move rule intervenes`;
    case "loss":
      return `${other} wins`;
  }
}
const short: Record<TablebaseOutcome, string> = {
  win: "Win",
  maybeWin: "Win?",
  cursedWin: "Win (50)",
  draw: "Draw",
  blessedLoss: "Loss (50)",
  maybeLoss: "Loss?",
  loss: "Loss",
};
const messages: Partial<Record<TablebaseResponse["status"], string>> = {
  noTables:
    "No tablebases. Choose a folder of Syzygy files in Options (Ctrl+Alt+O).",
  castling: "Tablebases hold no positions with castling rights.",
};

/**
 * Syzygy tablebases for the board: the position's result with the
 * distance to the next capture or pawn move (DTZ), and each move's result.
 */
export function TablebasePane({
  api,
  fen,
  onPlay,
}: {
  api: WorkbenchApi;
  fen?: string;
  onPlay: (uci: string) => void;
}) {
  const [result, setResult] = useState<TablebaseResponse>();
  const [error, setError] = useState<string>();
  const gate = useRef(new SelectionGate()).current;
  useEffect(() => {
    if (!fen) return;
    const request = gate.begin();
    void api
      .probeTablebase(fen, request)
      .then((r) => {
        if (!gate.accept(r.requestId)) return;
        setResult(r);
        setError(undefined);
      })
      .catch((e) => {
        if (gate.accept(request))
          setError((e as { message?: string })?.message ?? String(e));
      });
  }, [api, fen]);
  const mover: Color = fen?.split(" ")[1] === "b" ? "black" : "white";
  let message = error ?? (result && messages[result.status]);
  if (!message && result?.status === "tooManyPieces")
    message = `${result.pieces} pieces: the tables cover at most ${result.maxPieces}.`;
  if (!message && result && result.status !== "found")
    message = result.message ?? "This position is not in the tablebases.";
  return (
    <div className="tablebase-pane" aria-label="Tablebase">
      <div className="pane-header">
        <strong>Tablebase</strong>
        {result?.status === "found" && result.outcome && (
          <span className="hint" role="status">
            {outcomeText(result.outcome, mover)}
            {result.dtz ? ` · DTZ ${Math.abs(result.dtz)}` : ""}
          </span>
        )}
      </div>
      {message ? (
        <div className="empty-state">{message}</div>
      ) : (
        <div className="game-list book-list">
          <table aria-label="Tablebase moves">
            <thead>
              <tr>
                <th>Move</th>
                <th>Result</th>
                <th
                  className="num"
                  title="Plies to the next capture or pawn move"
                >
                  DTZ
                </th>
              </tr>
            </thead>
            <tbody>
              {result?.moves.map((m) => (
                <tr
                  key={m.uci}
                  data-outcome={m.outcome}
                  onClick={() => onPlay(m.uci)}
                >
                  <td>
                    <button
                      className="book-move"
                      aria-label={`Play ${m.san}`}
                      onClick={(ev) => {
                        ev.stopPropagation();
                        onPlay(m.uci);
                      }}
                    >
                      {m.san}
                    </button>
                  </td>
                  <td title={outcomeText(m.outcome, mover)}>
                    {m.ends === "mate"
                      ? "Mate"
                      : m.ends === "stalemate"
                        ? "Stalemate"
                        : short[m.outcome]}
                  </td>
                  <td className="num">
                    {m.zeroing
                      ? "Zeroing"
                      : m.dtz !== undefined
                        ? `${m.rounded ? "~" : ""}${Math.abs(m.dtz)}`
                        : ""}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
