import { useState } from "react";
import type { AppSettings } from "../api/types";

/**
 * The reference app's Options (Ctrl+Alt+O): how many game tabs may be open at once
 * (the reference app's "Maximum Board Windows", 1 to 8), and the Syzygy tablebase
 * folder.
 */
export function OptionsDialog({
  settings,
  onApply,
  onClose,
  error,
  chooseFolder,
}: {
  settings: AppSettings;
  /** Asks for a folder; null when cancelled. */
  chooseFolder: () => Promise<string | string[] | null>;
  onApply: (settings: AppSettings) => void;
  onClose: () => void;
  error?: string;
}) {
  const [tabs, setTabs] = useState(String(settings.maxGameTabs));
  const [folder, setFolder] = useState(settings.tablebaseDir);
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
          if (valid)
            onApply({ ...settings, maxGameTabs: value, tablebaseDir: folder });
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
        <fieldset className="mask-row">
          <legend className="mask-label">Tablebases</legend>
          <span aria-label="Tablebase folder" className="path">
            {folder ?? "No folder"}
          </span>{" "}
          <button
            type="button"
            onClick={() =>
              void chooseFolder().then((p) => {
                if (typeof p === "string") setFolder(p);
              })
            }
          >
            Choose folder…
          </button>
          {folder && (
            <button type="button" onClick={() => setFolder(undefined)}>
              No tablebases
            </button>
          )}
          <p className="hint">
            A folder of Syzygy files (.rtbw, .rtbz). The Tablebase tab shows
            their results for the board.
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
