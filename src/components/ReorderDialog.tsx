/**
 * Reorder Variations asks first, as ChessBase does, whether the main line
 * may change too ("Including the mainline?").
 */
export function ReorderDialog({
  onAnswer,
  onClose,
}: {
  onAnswer: (mainLine: boolean) => void;
  onClose: () => void;
}) {
  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Reorder Variations"
        className="modal maintenance"
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Reorder Variations</h2>
        <p>Including the mainline?</p>
        <p className="hint">
          At every branch from the current move on, the line with the most moves
          becomes the main continuation, so variations nest as little as
          possible. Without the main line, only the variations are reordered.
          Undo restores the order.
        </p>
        <div className="mask-buttons">
          <button type="button" onClick={() => onAnswer(true)}>
            Yes
          </button>
          <button type="button" onClick={() => onAnswer(false)}>
            No
          </button>
          <button type="button" onClick={onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
