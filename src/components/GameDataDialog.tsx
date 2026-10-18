import { useState } from "react";

const FIELDS: [tag: string, label: string][] = [
  ["White", "White"],
  ["WhiteElo", "Elo White"],
  ["Black", "Black"],
  ["BlackElo", "Elo Black"],
  ["Event", "Tournament"],
  ["Site", "Site"],
  ["Round", "Round"],
  ["Date", "Date"],
  ["ECO", "ECO"],
  ["Annotator", "Annotator"],
];
const RESULTS: [string, string][] = [
  ["1-0", "1-0"],
  ["0-1", "0-1"],
  ["1/2-1/2", "½-½"],
  ["*", "Line / unknown"],
];

/**
 * The reference app's Edit Game Data (F2): the game's players, ratings, tournament,
 * date, result and ECO, saved as PGN tags. Other tags are kept.
 */
export function GameDataDialog({
  tags,
  onApply,
  onClose,
}: {
  tags: Record<string, string>;
  onApply: (tags: Record<string, string>) => void;
  onClose: () => void;
}) {
  const [values, setValues] = useState<Record<string, string>>(() => {
    const shown = { ...tags };
    // "?" is PGN for unknown; the fields show it as empty.
    for (const [tag] of FIELDS) if (shown[tag] === "?") shown[tag] = "";
    return shown;
  });
  const set = (tag: string, value: string) =>
    setValues((v) => ({ ...v, [tag]: value }));
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Game data"
        className="modal game-data"
        onSubmit={(e) => {
          e.preventDefault();
          onApply(values);
        }}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Game data</h2>
        <div className="game-data-grid">
          {FIELDS.map(([tag, label]) => (
            <label key={tag}>
              <span>{label}</span>
              <input
                aria-label={label}
                value={values[tag] ?? ""}
                placeholder={tag === "Date" ? "YYYY.MM.DD" : undefined}
                inputMode={tag.endsWith("Elo") ? "numeric" : undefined}
                autoFocus={tag === "White"}
                onChange={(e) => set(tag, e.target.value)}
              />
            </label>
          ))}
        </div>
        <fieldset className="mask-row">
          <legend className="mask-label">Result</legend>
          {RESULTS.map(([value, label]) => (
            <label key={value} className="mask-check">
              <input
                type="radio"
                name="result"
                checked={(values.Result ?? "*") === value}
                onChange={() => set("Result", value)}
              />
              {label}
            </label>
          ))}
        </fieldset>
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
