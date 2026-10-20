import type { MaterialQuery, MaterialRange, MaterialSide } from "../api/types";

const kinds = [
  "queens",
  "rooks",
  "bishops",
  "knights",
  "pawns",
  "minors",
  "total",
] as const;
const names = [
  "queens",
  "rooks",
  "bishops",
  "knights",
  "pawns",
  "minor pieces",
  "total men",
];

export function materialError(q?: MaterialQuery): string | undefined {
  if (!q) return;
  const valid = (r: MaterialRange | undefined, low: number, high: number) =>
    !r ||
    ([r.min, r.max].every(
      (v) => v === undefined || (Number.isInteger(v) && v >= low && v <= high),
    ) &&
      (r.min === undefined || r.max === undefined || r.min <= r.max));
  if (
    [q.white, q.black].some((side) =>
      kinds.some((k) => !valid(side?.[k], 0, 16)),
    ) ||
    !valid(q.balance, -144, 144)
  )
    return "Enter a valid material range: minimum must not exceed maximum.";
  if (
    [q.first, q.last, q.length].some(
      (v) => v !== undefined && (!Number.isInteger(v) || v < 1 || v > 65535),
    ) ||
    (q.first !== undefined && q.last !== undefined && q.first > q.last)
  )
    return "Enter a valid move range and a length of at least 1.";
}

function preset(name: string): MaterialQuery {
  const side: MaterialSide = {};
  const roles =
    name === "minor"
      ? ["bishops", "knights"]
      : [name === "pawn" ? "pawns" : `${name}s`];
  if (name !== "any") {
    for (const k of kinds.slice(0, 4))
      if (!roles.includes(k)) side[k] = { max: 0 };
    if (name === "minor") side.minors = { min: 1 };
    else if (name !== "pawn") side[roles[0] as keyof MaterialSide] = { min: 1 };
  }
  return { white: structuredClone(side), black: structuredClone(side) };
}

function currentPreset(q: MaterialQuery): string {
  for (const name of ["pawn", "rook", "queen", "bishop", "knight", "minor"]) {
    const expected = preset(name);
    if (
      (["white", "black"] as const).every((side) =>
        kinds.every(
          (kind) =>
            q[side]?.[kind]?.min === expected[side]?.[kind]?.min &&
            q[side]?.[kind]?.max === expected[side]?.[kind]?.max,
        ),
      )
    )
      return name;
  }
  return "any";
}

export function MaterialMask({
  value,
  onChange,
}: {
  value?: MaterialQuery;
  onChange: (q: MaterialQuery | undefined) => void;
}) {
  const q = value ?? {};
  const set = (patch: Partial<MaterialQuery>) => onChange({ ...q, ...patch });
  const range = (
    label: string,
    value: MaterialRange | undefined,
    change: (r: MaterialRange) => void,
    signed = false,
  ) => (
    <>
      {(["min", "max"] as const).map((bound, i) => (
        <input
          key={bound}
          type="number"
          step="1"
          min={signed ? -144 : 0}
          max={signed ? 144 : 16}
          aria-label={`${label} ${i ? "to" : "from"}`}
          placeholder={i ? "max" : "min"}
          value={value?.[bound] ?? ""}
          onChange={(e) =>
            change({
              ...value,
              [bound]:
                e.target.value === "" ? undefined : Number(e.target.value),
            })
          }
        />
      ))}
    </>
  );
  return (
    <div className="material-mask">
      <label className="mask-check">
        <input
          type="checkbox"
          checked={!!value}
          onChange={(e) => onChange(e.target.checked ? {} : undefined)}
        />
        Use material criteria
      </label>
      <fieldset disabled={!value}>
        <label className="mask-row">
          Endgame type{" "}
          <select
            aria-label="Endgame type"
            value={currentPreset(q)}
            onChange={(e) => onChange({ ...q, ...preset(e.target.value) })}
          >
            <option value="any">Any material / custom</option>
            <option value="pawn">Pawn endings</option>
            <option value="rook">Rook endings</option>
            <option value="queen">Queen endings</option>
            <option value="bishop">Bishop endings</option>
            <option value="knight">Knight endings</option>
            <option value="minor">Minor-piece endings</option>
          </select>
        </label>
        <table>
          <thead>
            <tr>
              <th>Pieces</th>
              <th colSpan={2}>White (from – to)</th>
              <th colSpan={2}>Black (from – to)</th>
            </tr>
          </thead>
          <tbody>
            {kinds.map((kind, i) => (
              <tr key={kind}>
                <th scope="row">{names[i]}</th>
                {(["white", "black"] as const).map((side) => (
                  <td key={side} colSpan={2}>
                    {range(
                      `${side === "white" ? "White" : "Black"} ${names[i]}`,
                      q[side]?.[kind],
                      (r) => set({ [side]: { ...q[side], [kind]: r } }),
                    )}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
        <p className="hint">
          Blank bounds allow any count. Minor pieces = bishops + knights. Total
          men excludes kings.
        </p>
        <div className="mask-row">
          <span>Material balance</span>
          {range(
            "Material balance",
            q.balance,
            (r) => set({ balance: r }),
            true,
          )}
        </div>
        <p className="hint">
          White minus Black in pawn units: Q=9, R=5, B=N=3, P=1. Negative means
          Black is ahead.
        </p>
        <label className="mask-check">
          <input
            type="checkbox"
            checked={!!q.ignoreColors}
            onChange={(e) => set({ ignoreColors: e.target.checked })}
          />
          Ignore material colours
        </label>
        <div className="mask-row">
          {(["first", "last", "length"] as const).map((k) => (
            <label key={k}>
              {k === "first"
                ? "First move"
                : k === "last"
                  ? "Last move"
                  : "Consecutive positions"}
              <input
                type="number"
                step="1"
                min="1"
                max="65535"
                aria-label={`Material ${k}`}
                value={q[k] ?? ""}
                placeholder={k === "last" ? "any" : "1"}
                onChange={(e) =>
                  set({
                    [k]:
                      e.target.value === ""
                        ? undefined
                        : Number(e.target.value),
                  })
                }
              />
            </label>
          ))}
        </div>
        <label className="mask-check">
          <input
            type="checkbox"
            checked={!!q.lines}
            onChange={(e) => set({ lines: e.target.checked })}
          />
          Include material variations
        </label>
        <p className="hint">
          Searches positions along each game; opens the first matching run. Move
          numbers count from the game’s start. Replays candidate games, with
          progress and Cancel.
        </p>
      </fieldset>
      {materialError(value) && <p role="alert">{materialError(value)}</p>}
    </div>
  );
}
