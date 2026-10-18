import { useState } from "react";
import type { SelectionNames } from "../api/types";

/**
 * Edit Game Data for a selection (the reference app's F2 in the list, and its Set
 * Tournament, Set Annotator and Set Source): the fields ticked are set for
 * every selected game. They start with the first selected game's values,
 * as in the reference app; an empty field clears it.
 */
export function SelectionDataDialog({
  games,
  initial,
  running,
  error,
  onStart,
  onCancel,
  onClose,
}: {
  games: number;
  initial: {
    event?: string;
    site?: string;
    annotator?: string;
    source?: string;
  };
  running?: string;
  error?: string;
  onStart: (names: SelectionNames) => void;
  onCancel: () => void;
  onClose: () => void;
}) {
  const [set, setSet] = useState({
    tournament: false,
    annotator: false,
    source: false,
  });
  const [event, setEvent] = useState(initial.event ?? "");
  const [site, setSite] = useState(initial.site ?? "");
  const [annotator, setAnnotator] = useState(initial.annotator ?? "");
  const [title, setTitle] = useState(initial.source ?? "");
  const [publisher, setPublisher] = useState("");
  const chosen = set.tournament || set.annotator || set.source;
  const tick = (key: keyof typeof set) => (
    <input
      type="checkbox"
      aria-label={`Set ${key}`}
      checked={set[key]}
      onChange={(e) => setSet((s) => ({ ...s, [key]: e.target.checked }))}
    />
  );
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Edit Game Data"
        className="modal maintenance"
        onSubmit={(e) => {
          e.preventDefault();
          if (!chosen || running) return;
          onStart({
            ...(set.tournament ? { tournament: [site, event] } : {}),
            ...(set.annotator ? { annotator } : {}),
            ...(set.source ? { source: [title, publisher] } : {}),
          });
        }}
        onKeyDown={(e) => e.key === "Escape" && !running && onClose()}
      >
        <h2>Edit Game Data</h2>
        <p>
          The fields ticked are set for all {games} selected games. Only their
          names change; moves and annotations stay as they are.
        </p>
        <fieldset className="maintenance-options" disabled={!!running}>
          <div className="mask-row">
            {tick("tournament")}
            <span className="mask-label">Tournament</span>
            <input
              aria-label="Tournament"
              value={event}
              onChange={(e) => setEvent(e.target.value)}
            />
            <input
              aria-label="Site"
              placeholder="Site"
              value={site}
              onChange={(e) => setSite(e.target.value)}
            />
          </div>
          <div className="mask-row">
            {tick("annotator")}
            <span className="mask-label">Annotator</span>
            <input
              aria-label="Annotator"
              value={annotator}
              onChange={(e) => setAnnotator(e.target.value)}
            />
          </div>
          <div className="mask-row">
            {tick("source")}
            <span className="mask-label">Source</span>
            <input
              aria-label="Source"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
            <input
              aria-label="Publisher"
              placeholder="Publisher"
              value={publisher}
              onChange={(e) => setPublisher(e.target.value)}
            />
          </div>
        </fieldset>
        {running && (
          <p role="status" className="hint">
            Writing: {running}
          </p>
        )}
        {error && <p role="alert">{error}</p>}
        <div className="mask-buttons">
          {running ? (
            <button type="button" onClick={onCancel}>
              Stop
            </button>
          ) : (
            <>
              <button type="submit" disabled={!chosen}>
                Set
              </button>
              <button type="button" onClick={onClose}>
                Close
              </button>
            </>
          )}
        </div>
      </form>
    </div>
  );
}
