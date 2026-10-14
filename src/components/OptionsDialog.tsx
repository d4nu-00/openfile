import { useState } from "react";
import type { AppSettings } from "../api/types";

/**
 * ChessBase's Options (Ctrl+Alt+O). For now its Limits page: how many game
 * tabs may be open at once (ChessBase's "Maximum Board Windows", 1 to 8).
 */
export function OptionsDialog({
  settings,
  onApply,
  onClose,
  error,
}: {
  settings: AppSettings;
  onApply: (settings: AppSettings) => void;
  onClose: () => void;
  error?: string;
}) {
  const [tabs, setTabs] = useState(String(settings.maxGameTabs));
  const value = Number(tabs);
  const valid = Number.isInteger(value) && value >= 1 && value <= 8;
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Options"
        className="modal options"
        onSubmit={(e) => {
          e.preventDefault();
          if (valid) onApply({ ...settings, maxGameTabs: value });
        }}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Options</h2>
        <fieldset className="mask-row">
          <legend className="mask-label">Limits</legend>
          <label>
            <span>Maximum game tabs</span>{" "}
            <input
              type="number"
              min={1}
              max={8}
              aria-label="Maximum game tabs"
              value={tabs}
              autoFocus
              onChange={(e) => setTabs(e.target.value)}
            />
          </label>
          <p className="hint">
            Each game opened from a list gets its own tab. With the most tabs
            open, the least recently used tab without unsaved changes is reused.
            With 1, every game opens in the same tab.
          </p>
        </fieldset>
        {error && <p className="error">{error}</p>}
        <div className="mask-buttons">
          <button type="submit" disabled={!valid}>
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
