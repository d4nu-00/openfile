import { useState } from "react";
import type { DatabaseProperties, DatabaseSymbol } from "../api/types";
import { KINDS } from "../views/StartScreen";

export function DatabasePropertiesDialog({
  symbol,
  save,
  onClose,
}: {
  symbol: DatabaseSymbol;
  save: (properties: DatabaseProperties) => Promise<void>;
  onClose: () => void;
}) {
  const [name, setName] = useState(symbol.name);
  const [kind, setKind] = useState(symbol.kind);
  const [description, setDescription] = useState(symbol.description ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const Icon = (KINDS[kind] ?? KINDS.unspecific)[1];
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Database properties"
        className="modal database-properties"
        onKeyDown={(e) => {
          if (e.key === "Escape" && !busy) onClose();
        }}
        onSubmit={(e) => {
          e.preventDefault();
          if (busy || !name.trim()) return;
          setBusy(true);
          setError(undefined);
          void save({ name, kind, description })
            .then(onClose)
            .catch((e: unknown) =>
              setError((e as { message?: string })?.message ?? String(e)),
            )
            .finally(() => setBusy(false));
        }}
      >
        <h2>Database properties</h2>
        <p className="path">{symbol.path}</p>
        {!symbol.exists && (
          <p className="hint">
            The database file is missing. You can still edit its local
            properties.
          </p>
        )}
        <label>
          Name{" "}
          <input
            aria-label="Name"
            autoFocus
            maxLength={128}
            value={name}
            onChange={(e) => setName(e.target.value)}
            disabled={busy}
          />
        </label>
        <label className="database-kind-preview">
          <Icon aria-hidden /> Icon / type{" "}
          <select
            aria-label="Icon / type"
            value={kind}
            onChange={(e) => setKind(e.target.value)}
            disabled={busy}
          >
            {Object.entries(KINDS).map(([id, [label]]) => (
              <option key={id} value={id}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label>
          Description{" "}
          <textarea
            aria-label="Description"
            maxLength={2048}
            rows={4}
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            disabled={busy}
          />
        </label>
        <p className="hint">
          Saved in OpenFile settings on this computer. The name, icon / type and
          description do not travel with the database files.
        </p>
        {error && <p role="alert">{error}</p>}
        <div className="mask-buttons">
          <button type="submit" disabled={busy || !name.trim()}>
            {busy ? "Saving…" : "Save"}
          </button>
          <button type="button" disabled={busy} onClick={onClose}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}
