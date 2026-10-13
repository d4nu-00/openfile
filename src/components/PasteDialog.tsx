import { useState } from "react";

/**
 * Asked for when the clipboard cannot be read: PGN for Paste Game, FEN for
 * Paste Position.
 */
export function PasteDialog({
  kind,
  onPaste,
  onClose,
}: {
  kind: "game" | "position";
  onPaste: (text: string) => void;
  onClose: () => void;
}) {
  const [text, setText] = useState("");
  const label = kind === "game" ? "PGN" : "FEN";
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label={kind === "game" ? "Paste game" : "Paste position"}
        className="modal paste-dialog"
        onSubmit={(e) => {
          e.preventDefault();
          if (text.trim()) onPaste(text);
        }}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>{kind === "game" ? "Paste game" : "Paste position"}</h2>
        <p className="hint">
          The clipboard could not be read. Paste the {label} here.
        </p>
        <textarea
          aria-label={label}
          autoFocus
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
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
