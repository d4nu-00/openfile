import { useState } from "react";
import { emptyFilter, type EloMode, type GameFilter } from "../api/types";

/** Number input value: empty means "no bound". */
const num = (v: string) => (v.trim() === "" ? undefined : Number(v));
const text = (v?: number) => (v === undefined ? "" : String(v));

const RESULTS: [string, string][] = [
  ["1-0", "1-0"],
  ["0-1", "0-1"],
  ["1/2-1/2", "½-½"],
  ["0-0", "0-0"],
];

/**
 * ChessBase's search mask, game data tab: players, tournament, annotator,
 * Elo, year, ECO, moves and result. Searching filters the game list.
 */
export function SearchMask({
  initial,
  onSearch,
  onClose,
}: {
  initial?: GameFilter;
  onSearch: (filter: GameFilter) => void;
  onClose: () => void;
}) {
  const [f, setF] = useState<GameFilter>(initial ?? emptyFilter());
  const set = (patch: Partial<GameFilter>) => setF((v) => ({ ...v, ...patch }));
  const player = (side: "white" | "black", label: string) => (
    <div className="mask-row">
      <span className="mask-label">{label}</span>
      <input
        aria-label={`${label} last name`}
        placeholder="Last name"
        value={f[side].last}
        onChange={(e) => set({ [side]: { ...f[side], last: e.target.value } })}
      />
      <input
        aria-label={`${label} first name`}
        placeholder="First name"
        value={f[side].first}
        onChange={(e) => set({ [side]: { ...f[side], first: e.target.value } })}
      />
    </div>
  );
  const range = (
    label: string,
    min: keyof GameFilter,
    max: keyof GameFilter,
    inputMode: "numeric" | "text" = "numeric",
  ) => (
    <div className="mask-row">
      <span className="mask-label">{label}</span>
      <input
        aria-label={`${label} from`}
        inputMode={inputMode}
        value={
          inputMode === "text"
            ? (f[min] as string)
            : text(f[min] as number | undefined)
        }
        onChange={(e) =>
          set({
            [min]: inputMode === "text" ? e.target.value : num(e.target.value),
          })
        }
      />
      –
      <input
        aria-label={`${label} to`}
        inputMode={inputMode}
        value={
          inputMode === "text"
            ? (f[max] as string)
            : text(f[max] as number | undefined)
        }
        onChange={(e) =>
          set({
            [max]: inputMode === "text" ? e.target.value : num(e.target.value),
          })
        }
      />
    </div>
  );
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Search mask"
        className="modal search-mask"
        onSubmit={(e) => {
          e.preventDefault();
          onSearch(f);
        }}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Search mask</h2>
        {player("white", "White")}
        <label className="mask-check">
          <input
            type="checkbox"
            checked={f.winsOnly}
            onChange={(e) => set({ winsOnly: e.target.checked })}
          />
          Wins Only
        </label>
        {player("black", "Black")}
        <label className="mask-check">
          <input
            type="checkbox"
            checked={f.ignoreColors}
            onChange={(e) => set({ ignoreColors: e.target.checked })}
          />
          Ignore colours
        </label>
        <div className="mask-row">
          <span className="mask-label">Tournament</span>
          <input
            aria-label="Tournament"
            value={f.tournament}
            onChange={(e) => set({ tournament: e.target.value })}
          />
        </div>
        <div className="mask-row">
          <span className="mask-label">Annotator</span>
          <input
            aria-label="Annotator"
            value={f.annotator}
            onChange={(e) => set({ annotator: e.target.value })}
          />
        </div>
        {range("Elo", "eloMin", "eloMax")}
        <div className="mask-row" role="radiogroup" aria-label="Elo applies to">
          <span className="mask-label" />
          {(
            [
              ["none", "None"],
              ["one", "One"],
              ["both", "Both"],
              ["average", "Av"],
            ] as [EloMode, string][]
          ).map(([mode, label]) => (
            <label key={mode} className="mask-check">
              <input
                type="radio"
                name="elo-mode"
                checked={f.eloMode === mode}
                onChange={() => set({ eloMode: mode })}
              />
              {label}
            </label>
          ))}
        </div>
        {range("Year", "yearMin", "yearMax")}
        {range("ECO", "ecoFrom", "ecoTo", "text")}
        {range("Moves", "movesMin", "movesMax")}
        <fieldset className="mask-row">
          <legend className="mask-label">Result</legend>
          {RESULTS.map(([value, label]) => (
            <label key={value} className="mask-check">
              <input
                type="checkbox"
                checked={f.results.includes(value)}
                onChange={(e) =>
                  set({
                    results: e.target.checked
                      ? [...f.results, value]
                      : f.results.filter((r) => r !== value),
                  })
                }
              />
              {label}
            </label>
          ))}
        </fieldset>
        <label className="mask-check">
          <input
            type="checkbox"
            checked={f.excludeDeleted}
            onChange={(e) => set({ excludeDeleted: e.target.checked })}
          />
          Skip deleted games
        </label>
        <div className="mask-buttons">
          <button type="submit" autoFocus>
            Search
          </button>
          <button type="button" onClick={() => setF(emptyFilter())}>
            Reset
          </button>
          <button type="button" onClick={onClose}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}
