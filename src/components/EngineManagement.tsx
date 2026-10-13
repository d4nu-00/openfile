import { useState } from "react";
import type { AppSettings, InstalledEngine } from "../api/types";

/**
 * ChessBase's Engine Management: the installed UCI engines, the default
 * engine (for Default Kibitzer, Alt+F2), and starting one as a kibitzer.
 * The list is kept in the app's settings.
 */
export function EngineManagement({
  settings,
  onSave,
  onAdd,
  onStart,
  onClose,
  error,
}: {
  settings: AppSettings;
  onSave: (next: AppSettings) => void;
  /** Asks for an executable and names it from its UCI handshake. */
  onAdd: () => void;
  onStart: (engine: InstalledEngine) => void;
  onClose: () => void;
  error?: string;
}) {
  const [chosen, setChosen] = useState(settings.engines[0]?.id);
  const engine = settings.engines.find((e) => e.id === chosen);
  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Engine Management"
        className="modal engine-management"
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Engine Management</h2>
        {settings.engines.length ? (
          <table aria-label="Installed engines">
            <thead>
              <tr>
                <th>Default</th>
                <th>Engine</th>
                <th>Executable</th>
              </tr>
            </thead>
            <tbody>
              {settings.engines.map((e) => (
                <tr
                  key={e.id}
                  aria-selected={e.id === chosen}
                  onClick={() => setChosen(e.id)}
                >
                  <td>
                    <input
                      type="radio"
                      name="default-engine"
                      aria-label={`Default engine ${e.name}`}
                      checked={settings.defaultEngine === e.id}
                      onChange={() =>
                        onSave({ ...settings, defaultEngine: e.id })
                      }
                    />
                  </td>
                  <td>{e.name}</td>
                  <td className="path" title={e.path}>
                    {e.path}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="empty-state">No engines installed yet.</p>
        )}
        {error && <p role="alert">{error}</p>}
        <div className="mask-buttons">
          <button type="button" onClick={onAdd}>
            Add engine…
          </button>
          <button
            type="button"
            disabled={!engine}
            onClick={() => engine && onStart(engine)}
          >
            Start as kibitzer
          </button>
          <button
            type="button"
            disabled={!engine}
            onClick={() =>
              engine &&
              onSave({
                ...settings,
                engines: settings.engines.filter((e) => e.id !== engine.id),
                defaultEngine:
                  settings.defaultEngine === engine.id
                    ? undefined
                    : settings.defaultEngine,
              })
            }
          >
            Remove
          </button>
          <button type="button" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
