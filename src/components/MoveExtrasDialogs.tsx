import { useState } from "react";
import { MEDAL_NAMES } from "../api/types";
import { packMoveTime } from "../state/moveExtras";

/** Set Medal: the reference app's sixteen medal bits for a move. */
export function MedalDialog({
  bits,
  onApply,
  onClose,
}: {
  bits: number;
  onApply: (bits: number) => void;
  onClose: () => void;
}) {
  const [value, setValue] = useState(bits);
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Set Medal"
        className="modal move-extra"
        onSubmit={(e) => {
          e.preventDefault();
          onApply(value);
        }}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Set Medal</h2>
        <div className="medal-grid">
          {MEDAL_NAMES.map((name, i) => (
            <label key={name}>
              <input
                type="checkbox"
                checked={((value >>> i) & 1) === 1}
                onChange={(e) =>
                  setValue(
                    e.target.checked
                      ? (value | (1 << i)) >>> 0
                      : value & ~(1 << i),
                  )
                }
              />{" "}
              {name}
            </label>
          ))}
        </div>
        <div className="modal-actions">
          <button type="button" onClick={() => setValue(0)}>
            Reset
          </button>
          <button type="submit">OK</button>
          <button type="button" onClick={onClose}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}

/** Move Time (Ctrl+J): the time used for the move, h:mm:ss. */
export function MoveTimeDialog({
  time,
  onApply,
  onClose,
}: {
  time?: string;
  onApply: (packed: number | null) => void;
  onClose: () => void;
}) {
  const [text, setText] = useState(time ?? "");
  const packed = packMoveTime(text);
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Move Time"
        className="modal move-extra"
        onSubmit={(e) => {
          e.preventDefault();
          if (packed !== undefined) onApply(packed);
        }}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Move Time</h2>
        <label className="mask-row">
          <span>Time used (h:mm:ss)</span>{" "}
          <input
            aria-label="Time used"
            value={text}
            autoFocus
            onChange={(e) => setText(e.target.value)}
          />
        </label>
        {text && packed === undefined && (
          <p role="alert">Enter the time as h:mm:ss, m:ss or seconds.</p>
        )}
        <div className="modal-actions">
          {time && (
            <button type="button" onClick={() => onApply(null)}>
              Remove
            </button>
          )}
          <button type="submit" disabled={packed === undefined}>
            OK
          </button>
          <button type="button" onClick={onClose}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}
