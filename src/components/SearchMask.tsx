import { useState } from "react";
import {
  emptyAnnotations,
  emptyFilter,
  MEDAL_NAMES,
  type AnnotationQuery,
  type EloMode,
  type GameFilter,
  type PositionQuery,
} from "../api/types";
import { PositionMask } from "./PositionMask";
import { SYMBOL_GROUPS } from "../state/symbols";

/** The Annotations tab's checkboxes, in dialog 343's order. */
const ANNOTATION_CHECKS: [keyof AnnotationQuery, string][] = [
  ["anyText", "Any text"],
  ["anySymbols", "Any symbols"],
  ["colours", "Colours"],
  ["training", "Training"],
  ["multimedia", "Multimedia"],
  ["pawnStructure", "Pawn structure"],
  ["piecePath", "Piece path"],
  ["variations", "Variations"],
  ["criticalOpening", "Critical opening position"],
  ["criticalMiddlegame", "Critical middlegame position"],
  ["criticalEndgame", "Critical endgame position"],
];

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
 * ChessBase's search mask. The game data tab has players, tournament,
 * annotator, Elo, year, ECO, moves, result, final position and text; the
 * position tab finds a pattern or a whole position. Searching filters the
 * game list.
 */
export function SearchMask({
  initial,
  initialTab,
  initialPosition,
  boardFen,
  onSearch,
  onSearchPosition,
  onClose,
}: {
  initial?: GameFilter;
  /** The tab to open on. */
  initialTab?: "annotations";
  /** The position tab of the current search. */
  initialPosition?: PositionQuery;
  /** The current board, offered on the position tab. */
  boardFen?: string;
  onSearch: (filter: GameFilter) => void;
  onSearchPosition: (query: PositionQuery) => void;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<
    "game" | "annotations" | "position" | "medals"
  >(initialTab ?? (initialPosition ? "position" : "game"));
  const [position, setPosition] = useState<{
    query?: PositionQuery;
    valid: boolean;
  }>({ valid: false });
  const [positionKey, setPositionKey] = useState(0);
  const [f, setF] = useState<GameFilter>(initial ?? emptyFilter());
  const set = (patch: Partial<GameFilter>) => setF((v) => ({ ...v, ...patch }));
  const anno = f.annotations ?? emptyAnnotations();
  const setAnno = (patch: Partial<AnnotationQuery>) =>
    set({ annotations: { ...anno, ...patch } });
  const medals = f.medals ?? 0;
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
        className={`modal search-mask${tab === "position" ? " wide" : ""}`}
        onSubmit={(e) => {
          e.preventDefault();
          if (tab === "position") {
            if (position.query && position.valid)
              onSearchPosition(position.query);
          } else onSearch(f);
        }}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Search mask</h2>
        <div className="pane-tabs mask-tabs" role="tablist">
          {(
            [
              ["game", "Game data"],
              ["annotations", "Annotations"],
              ["position", "Position"],
              ["medals", "Medals"],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={tab === id}
              onClick={() => setTab(id)}
            >
              {label}
            </button>
          ))}
        </div>
        {tab === "position" && (
          <PositionMask
            key={positionKey}
            initial={positionKey ? undefined : initialPosition}
            boardFen={boardFen}
            onChange={(query, valid) => setPosition({ query, valid })}
          />
        )}
        {tab === "annotations" && (
          <div className="mask-annotations">
            {(["text1", "text2"] as const).map((k, i) => (
              <div className="mask-row" key={k}>
                <span className="mask-label">Text {i + 1}</span>
                <input
                  aria-label={`Text ${i + 1}`}
                  value={anno[k]}
                  onChange={(e) => setAnno({ [k]: e.target.value })}
                />
              </div>
            ))}
            <label className="mask-check">
              <input
                type="checkbox"
                checked={anno.wholeWord}
                onChange={(e) => setAnno({ wholeWord: e.target.checked })}
              />
              Whole word
            </label>
            <fieldset className="mask-symbols">
              <legend className="mask-label">Symbols</legend>
              {Object.values(SYMBOL_GROUPS)
                .flat()
                .map((s) => (
                  <label key={s.nag} className="mask-check">
                    <input
                      type="checkbox"
                      checked={anno.symbols.includes(s.nag)}
                      onChange={(e) =>
                        setAnno({
                          symbols: e.target.checked
                            ? [...anno.symbols, s.nag]
                            : anno.symbols.filter((n) => n !== s.nag),
                        })
                      }
                    />
                    {s.name}
                  </label>
                ))}
            </fieldset>
            <div className="mask-checks">
              {ANNOTATION_CHECKS.map(([k, label]) => (
                <label key={k} className="mask-check">
                  <input
                    type="checkbox"
                    checked={!!anno[k]}
                    onChange={(e) => setAnno({ [k]: e.target.checked })}
                  />
                  {label}
                </label>
              ))}
            </div>
            <p className="hint">
              Every criterion set must hold. Annotations are read from each game
              that matches the other tabs, so this search is slower.
            </p>
          </div>
        )}
        {tab === "medals" && (
          <div className="mask-checks" role="group" aria-label="Medals">
            {MEDAL_NAMES.map((name, bit) => (
              <label key={name} className="mask-check">
                <input
                  type="checkbox"
                  checked={(medals & (1 << bit)) !== 0}
                  onChange={(e) =>
                    set({
                      medals: e.target.checked
                        ? medals | (1 << bit)
                        : medals & ~(1 << bit),
                    })
                  }
                />
                {name}
              </label>
            ))}
            <p className="hint">Games with any of the medals checked.</p>
          </div>
        )}
        {tab === "game" && (
          <>
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
            <div
              className="mask-row"
              role="radiogroup"
              aria-label="Elo applies to"
            >
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
            <div className="mask-row">
              <span className="mask-label">Difference</span>
              <input
                aria-label="Elo difference at most"
                inputMode="numeric"
                placeholder="max"
                value={text(f.eloDifference)}
                onChange={(e) => set({ eloDifference: num(e.target.value) })}
              />
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
            <fieldset className="mask-row">
              <legend className="mask-label">Final position</legend>
              {(
                [
                  ["mate", "Mate"],
                  ["stalemate", "Stalem."],
                  ["check", "Check"],
                ] as const
              ).map(([key, label]) => (
                <label key={key} className="mask-check">
                  <input
                    type="checkbox"
                    checked={!!f[key]}
                    onChange={(e) => set({ [key]: e.target.checked })}
                  />
                  {label}
                </label>
              ))}
            </fieldset>
            <div className="mask-row">
              <span className="mask-label">Full Text</span>
              <input
                aria-label="Full Text"
                value={f.fullText ?? ""}
                onChange={(e) => set({ fullText: e.target.value })}
              />
              <label className="mask-check">
                <input
                  type="checkbox"
                  checked={!!f.fullTextNot}
                  onChange={(e) => set({ fullTextNot: e.target.checked })}
                />
                NOT
              </label>
            </div>
            {(f.mate || f.stalemate || f.check || f.fullText?.trim()) && (
              <p className="hint">
                Final positions and text are read from each game that matches
                the other fields, so this search is slower.
              </p>
            )}
            <label className="mask-check">
              <input
                type="checkbox"
                checked={f.excludeDeleted}
                onChange={(e) => set({ excludeDeleted: e.target.checked })}
              />
              Skip deleted games
            </label>
          </>
        )}
        <div className="mask-buttons">
          <button
            type="submit"
            autoFocus
            disabled={tab === "position" && !position.valid}
          >
            Search
          </button>
          <button
            type="button"
            onClick={() => {
              setF(emptyFilter());
              setPositionKey((k) => k + 1);
            }}
          >
            Reset all
          </button>
          <button type="button" onClick={onClose}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}
