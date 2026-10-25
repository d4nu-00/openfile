import { useState } from "react";
import {
  emptyAnnotations,
  emptyFilter,
  emptyEnding,
  MEDAL_NAMES,
  type AnnotationQuery,
  type CountRange,
  type EloMode,
  type EndingQuery,
  type FinalMaterial,
  type GameFilter,
  type PieceCounts,
  type PositionQuery,
  type SearchScope,
} from "../api/types";
import { MaterialMask, materialError } from "./MaterialMask";
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

/** A count range as its select value: "any", "2" or "2+". */
const rangeValue = (r: CountRange) =>
  r.min !== undefined && r.min === r.max
    ? String(r.min)
    : r.min !== undefined && r.max === undefined
      ? `${r.min}+`
      : "any";
const parseRange = (v: string): CountRange =>
  v === "any"
    ? {}
    : v.endsWith("+")
      ? { min: Number(v.slice(0, -1)) }
      : { min: Number(v), max: Number(v) };

/** The piece kinds of an Ending tab row; bishops and knights are shown
 * either apart or pooled as minor pieces. */
const KINDS: [keyof PieceCounts, string, string, number][] = [
  ["queens", "Q", "queens", 3],
  ["rooks", "R", "rooks", 3],
  ["bishops", "B", "bishops", 3],
  ["knights", "N", "knights", 3],
  ["minors", "Minor", "minor pieces", 3],
  ["pawns", "P", "pawns", 8],
];

/** A select of piece counts: any, 0..max, 1+ and 2+ (the last two only when
 * `plus`). */
function CountSelect({
  label,
  value,
  max,
  plus = true,
  disabled,
  onChange,
}: {
  label: string;
  value: CountRange;
  max: number;
  plus?: boolean;
  disabled?: boolean;
  onChange: (r: CountRange) => void;
}) {
  const options = [
    "any",
    ...Array.from({ length: max + 1 }, (_, n) => String(n)),
    ...(plus ? ["1+", "2+"] : []),
  ];
  return (
    <select
      aria-label={label}
      disabled={disabled}
      value={rangeValue(value)}
      onChange={(e) => onChange(parseRange(e.target.value))}
    >
      {options.map((o) => (
        <option key={o}>{o}</option>
      ))}
    </select>
  );
}

const RESULTS: [string, string][] = [
  ["1-0", "1-0"],
  ["0-1", "0-1"],
  ["1/2-1/2", "½-½"],
  ["0-0", "0-0"],
];

/**
 * The reference app's search mask. The game data tab has players, tournament,
 * annotator, Elo, year, ECO, moves, result, final position, final material
 * and text; the ending tab picks final piece counts and the pieces that mate; the
 * position tab finds a pattern or a whole position. Searching filters the
 * game list.
 */
export function SearchMask({
  initial,
  initialTab,
  initialPosition,
  boardFen,
  resultCount,
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
  resultCount?: string;
  onSearch: (filter: GameFilter, scope: SearchScope) => void;
  onSearchPosition: (query: PositionQuery, scope: SearchScope) => void;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<
    "game" | "annotations" | "ending" | "position" | "material" | "medals"
  >(initialTab ?? (initialPosition ? "position" : "game"));
  const [scope, setScope] = useState<SearchScope>("all");
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
  const material: FinalMaterial = f.finalMaterial ?? {
    equal: false,
    separateMinors: false,
    countPawns: false,
  };
  const setMaterial = (patch: Partial<FinalMaterial>) =>
    set({ finalMaterial: { ...material, ...patch } });
  const mat = f.ending ?? emptyEnding();
  const setMat = (patch: Partial<EndingQuery>) =>
    set({ ending: { ...mat, ...patch } });
  // Bishops and knights apart when the filter says so.
  const [pooled, setPooled] = useState(
    ![mat.white, mat.black, mat.mateBy].some(
      (c) => rangeValue(c.bishops) !== "any" || rangeValue(c.knights) !== "any",
    ),
  );
  const poolMinors = (on: boolean) => {
    setPooled(on);
    // Only the shown kinds count.
    const fix = (c: PieceCounts): PieceCounts =>
      on ? { ...c, bishops: {}, knights: {} } : { ...c, minors: {} };
    setMat({
      white: fix(mat.white),
      black: fix(mat.black),
      mateBy: fix(mat.mateBy),
    });
  };
  const kinds = KINDS.filter(([k]) =>
    pooled ? k !== "bishops" && k !== "knights" : k !== "minors",
  );
  const matSet =
    mat.mateByEnabled ||
    [mat.white, mat.black].some(
      (c) =>
        c.othersZero ||
        KINDS.some(([k]) => rangeValue(c[k] as CountRange) !== "any") ||
        rangeValue(c.kings) !== "any",
    );
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
          if (tab !== "position" && materialError(f.material)) return;
          if (tab === "position") {
            if (position.query && position.valid)
              onSearchPosition(position.query, scope);
          } else onSearch(f, scope);
        }}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Search mask</h2>
        <div className="pane-tabs mask-tabs" role="tablist">
          {(
            [
              ["game", "Game data"],
              ["annotations", "Annotations"],
              ["ending", "Ending"],
              ["position", "Position"],
              ["material", "Material"],
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
        {tab === "material" && (
          <MaterialMask
            value={f.material}
            onChange={(material) => set({ material })}
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
        {tab === "ending" && (
          <div className="mask-ending">
            <fieldset className="mask-ending-block">
              <legend>Final position</legend>
              <div
                className="mask-ending-grid"
                style={{
                  gridTemplateColumns: `auto repeat(${kinds.length}, 1fr)`,
                }}
              >
                <span />
                {kinds.map(([k, head]) => (
                  <span key={k} className="mask-ending-head">
                    {head}
                  </span>
                ))}
                {(["white", "black"] as const).map((side) => {
                  const name = side === "white" ? "White" : "Black";
                  return [
                    <span key={side}>{name}</span>,
                    ...kinds.map(([k, , noun, max]) => (
                      <CountSelect
                        key={`${side}-${k}`}
                        label={`${name} ${noun}`}
                        value={mat[side][k] as CountRange}
                        max={max}
                        onChange={(r) =>
                          setMat({ [side]: { ...mat[side], [k]: r } })
                        }
                      />
                    )),
                  ];
                })}
              </div>
              <div className="mask-checks">
                <label className="mask-check">
                  <input
                    type="checkbox"
                    checked={mat.eitherColour}
                    onChange={(e) => setMat({ eitherColour: e.target.checked })}
                  />
                  Either colour
                </label>
                <label className="mask-check">
                  <input
                    type="checkbox"
                    checked={pooled}
                    onChange={(e) => poolMinors(e.target.checked)}
                  />
                  Bishop = Knight
                </label>
                <label className="mask-check">
                  <input
                    type="checkbox"
                    checked={mat.white.othersZero && mat.black.othersZero}
                    onChange={(e) =>
                      setMat({
                        white: { ...mat.white, othersZero: e.target.checked },
                        black: { ...mat.black, othersZero: e.target.checked },
                      })
                    }
                  />
                  No other pieces
                </label>
              </div>
            </fieldset>
            <fieldset className="mask-ending-block">
              <legend>Mate by</legend>
              <label className="mask-check">
                <input
                  type="checkbox"
                  checked={mat.mateByEnabled}
                  onChange={(e) => setMat({ mateByEnabled: e.target.checked })}
                />
                Final position is mate by
              </label>
              <div
                className="mask-ending-grid"
                style={{
                  gridTemplateColumns: `repeat(${kinds.length + 1}, 1fr)`,
                }}
              >
                <span className="mask-ending-head">K</span>
                {kinds.map(([k, head]) => (
                  <span key={k} className="mask-ending-head">
                    {head}
                  </span>
                ))}
                <CountSelect
                  label="Mating king"
                  value={mat.mateBy.kings}
                  max={1}
                  plus={false}
                  disabled={!mat.mateByEnabled}
                  onChange={(r) =>
                    setMat({ mateBy: { ...mat.mateBy, kings: r } })
                  }
                />
                {kinds.map(([k, , noun, max]) => (
                  <CountSelect
                    key={k}
                    label={`Mating ${noun}`}
                    value={mat.mateBy[k] as CountRange}
                    max={max}
                    disabled={!mat.mateByEnabled}
                    onChange={(r) =>
                      setMat({ mateBy: { ...mat.mateBy, [k]: r } })
                    }
                  />
                ))}
              </div>
              <label className="mask-check">
                <input
                  type="checkbox"
                  aria-label="Mate by: no other pieces"
                  disabled={!mat.mateByEnabled}
                  checked={mat.mateBy.othersZero}
                  onChange={(e) =>
                    setMat({
                      mateBy: { ...mat.mateBy, othersZero: e.target.checked },
                    })
                  }
                />
                No other pieces
              </label>
            </fieldset>
            {matSet && (
              <p className="hint">
                Final positions are read from each game that matches the other
                tabs, so this search is slower.
              </p>
            )}
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
              <legend>Final position</legend>
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
            <fieldset className="mask-row">
              <legend className="mask-label">Final material</legend>
              <label className="mask-check">
                <input
                  type="checkbox"
                  checked={material.equal}
                  onChange={(e) => setMaterial({ equal: e.target.checked })}
                />
                Equal pieces
              </label>
              <label className="mask-check">
                <input
                  type="checkbox"
                  disabled={!material.equal}
                  checked={!material.separateMinors}
                  onChange={(e) =>
                    setMaterial({ separateMinors: !e.target.checked })
                  }
                />
                Bishop = Knight
              </label>
              <label className="mask-check">
                <input
                  type="checkbox"
                  disabled={!material.equal}
                  checked={material.countPawns}
                  onChange={(e) =>
                    setMaterial({ countPawns: e.target.checked })
                  }
                />
                Count pawns
              </label>
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
            {(f.mate ||
              f.stalemate ||
              f.check ||
              material.equal ||
              f.fullText?.trim()) && (
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
        <fieldset className="mask-scope">
          <legend>Search target</legend>
          {(
            [
              ["all", "All games"],
              ["within", "Within results"],
              ["add", "Add to results"],
            ] as const
          ).map(([id, label]) => (
            <label key={id} className="mask-check">
              <input
                type="radio"
                name="search-scope"
                checked={scope === id}
                disabled={id !== "all" && resultCount === undefined}
                onChange={() => setScope(id)}
              />
              {label}
            </label>
          ))}
          <p className="hint">
            {resultCount === undefined
              ? "Search all games to create results first."
              : `${resultCount} current results. Within keeps matching entries; Add keeps existing entries and adds new matches without duplicates.`}
          </p>
        </fieldset>
        <div className="mask-buttons">
          <button
            type="submit"
            autoFocus
            disabled={
              (tab !== "position" && !!materialError(f.material)) ||
              (tab === "position" && !position.valid)
            }
          >
            Search
          </button>
          <button
            type="button"
            onClick={() => {
              setF(emptyFilter());
              setPooled(true);
              setScope("all");
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
