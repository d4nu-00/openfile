import { useState } from "react";
import type { Classification } from "../api/types";

/** "after 5...a6" style: the move number of a ply and who made it. */
export function afterPly(ply: number) {
  const move = Math.ceil(ply / 2);
  return ply % 2 ? `move ${move} (White)` : `move ${move} (Black)`;
}

/**
 * Classify Opening: the shown game's ECO code and opening name from the
 * workbench's open table, with the code the game has, and Set ECO to store
 * the new one in the game data.
 */
export function ClassifyDialog({
  result,
  onSetEco,
  onClose,
}: {
  result: Classification;
  onSetEco?: () => void;
  onClose: () => void;
}) {
  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Classify Opening"
        className="modal classify-opening"
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Classify Opening</h2>
        {result.eco ? (
          <>
            <p className="classify-code">
              <strong>{result.eco}</strong> {result.name}
            </p>
            {result.ply !== undefined && (
              <p className="hint">Reached after {afterPly(result.ply)}.</p>
            )}
          </>
        ) : (
          <p>
            The opening table does not know this game (a set-up position, or no
            moves).
          </p>
        )}
        <p className="hint">
          The game's ECO code: {result.current ?? "none"}. Codes and names come
          from the open lichess opening table (CC0).
        </p>
        <div className="mask-buttons">
          {onSetEco && (
            <button type="button" onClick={onSetEco}>
              Set ECO {result.eco}
            </button>
          )}
          <button type="button" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

/** Set ECO Codes: whether games that have a code keep it. */
export function SetEcoDialog({
  onStart,
  onClose,
}: {
  onStart: (onlyMissing: boolean) => void;
  onClose: () => void;
}) {
  const [onlyMissing, setOnlyMissing] = useState(true);
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Set ECO Codes"
        className="modal maintenance"
        onSubmit={(e) => {
          e.preventDefault();
          onStart(onlyMissing);
        }}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Set ECO Codes</h2>
        <label className="mask-check">
          <input
            type="checkbox"
            checked={onlyMissing}
            onChange={(e) => setOnlyMissing(e.target.checked)}
          />
          Only games without an ECO code
        </label>
        <p className="hint">
          Every game's main line is classified with the open lichess opening
          table. It agrees with the codes already stored for about three games
          in four, so codes already set are kept unless this is unticked. A code
          whose main part does not change keeps its stored sub-classification.
        </p>
        <div className="mask-buttons">
          <button type="submit">Start</button>
          <button type="button" onClick={onClose}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}
