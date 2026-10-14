import type { ScoreDto, Color } from "../api/types";
import { SelectionGate } from "./selection";
export function formatScore(
  score: ScoreDto,
  root: Color,
  perspective: Color,
): string {
  const sign = root === perspective ? 1 : -1;
  const value = (score.kind === "cp" ? score.value : score.moves) * sign;
  const bound =
    score.bound === "lowerbound"
      ? sign === 1
        ? "≥"
        : "≤"
      : score.bound === "upperbound"
        ? sign === 1
          ? "≤"
          : "≥"
        : "";
  return (
    bound +
    (score.kind === "cp"
      ? `${value >= 0 ? "+" : ""}${(value / 100).toFixed(2)}`
      : `${value >= 0 ? "+" : "-"}M${Math.abs(value)}`)
  );
}
export class AnalysisGate {
  private gate = new SelectionGate();
  private active?: { requestId: string; documentId: string; nodeId: number };
  begin(documentId: string, nodeId: number) {
    const requestId = this.gate.begin();
    this.active = { requestId, documentId, nodeId };
    return requestId;
  }
  accept(requestId: string) {
    return this.active?.requestId === requestId;
  }
  invalidate() {
    this.gate.begin();
    this.active = undefined;
  }
}
