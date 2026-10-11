import { useState } from "react";
import type { EngineOption } from "../api/types";
function Option({
  option,
  disabled,
  apply,
}: {
  option: EngineOption;
  disabled: boolean;
  apply: (n: string, v: string) => Promise<void>;
}) {
  const [value, setValue] = useState(option.default ?? ""),
    [error, setError] = useState("");
  function submit() {
    if (value.includes("\n") || value.includes("\r")) {
      setError("Option values cannot contain newlines.");
      return;
    }
    if (
      option.kind === "spin" &&
      (!/^-?\d+$/.test(value) ||
        !Number.isSafeInteger(Number(value)) ||
        Number(value) < (option.min ?? -Infinity) ||
        Number(value) > (option.max ?? Infinity))
    ) {
      setError(
        `Enter an integer between ${option.min ?? "−∞"} and ${option.max ?? "∞"}.`,
      );
      return;
    }
    if (option.kind === "combo" && !option.vars.includes(value)) {
      setError("Choose an advertised value.");
      return;
    }
    setError("");
    void apply(option.name, option.kind === "button" ? "" : value);
  }
  const rulesOption = ["uci_chess960", "uci_variant"].includes(
    option.name.toLowerCase(),
  );
  return (
    <div
      className="engine-option"
      title={rulesOption ? "Standard chess only" : undefined}
    >
      <label>
        {option.name}
        {option.kind === "check" ? (
          <input
            disabled={disabled || rulesOption}
            type="checkbox"
            checked={value === "true"}
            onChange={(e) => setValue(String(e.target.checked))}
          />
        ) : option.kind === "combo" ? (
          <select
            disabled={disabled || rulesOption}
            value={value}
            onChange={(e) => setValue(e.target.value)}
          >
            {option.vars.map((v) => (
              <option key={v}>{v}</option>
            ))}
          </select>
        ) : option.kind !== "button" ? (
          <input
            disabled={disabled || rulesOption}
            type={option.kind === "spin" ? "number" : "text"}
            value={value}
            onChange={(e) => setValue(e.target.value)}
          />
        ) : null}
      </label>
      <button disabled={disabled || rulesOption} onClick={submit}>
        {option.kind === "button" ? option.name : `Apply ${option.name}`}
      </button>
      {error && <span role="alert">{error}</span>}
    </div>
  );
}
export function EngineOptions({
  options,
  disabled,
  apply,
}: {
  options: EngineOption[];
  disabled: boolean;
  apply: (n: string, v: string) => Promise<void>;
}) {
  return (
    <details className="engine-options">
      <summary>Engine options</summary>
      {options.map((option) => (
        <Option
          key={option.name}
          option={option}
          disabled={disabled}
          apply={apply}
        />
      ))}
    </details>
  );
}
