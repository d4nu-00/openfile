import { useEffect, useRef, type KeyboardEvent } from "react";
import type { EntrySummary } from "../api/types";

const columns: {
  key: string;
  label: string;
  className?: string;
  value: (e: EntrySummary) => string | number | undefined;
}[] = [
  {
    key: "white",
    label: "White",
    value: (e) => (e.kind === "game" ? (e.white ?? "?") : "Text entry"),
  },
  {
    key: "whiteElo",
    label: "Elo W",
    className: "num",
    value: (e) => e.whiteElo || "",
  },
  {
    key: "black",
    label: "Black",
    value: (e) => e.black ?? (e.kind === "game" ? "?" : ""),
  },
  {
    key: "blackElo",
    label: "Elo B",
    className: "num",
    value: (e) => e.blackElo || "",
  },
  {
    key: "result",
    label: "Result",
    className: "center",
    value: (e) => e.result ?? "",
  },
  { key: "eco", label: "ECO", value: (e) => e.eco ?? "" },
  { key: "event", label: "Tournament", value: (e) => e.event ?? "" },
  {
    key: "round",
    label: "Round",
    className: "center",
    value: (e) => e.round ?? "",
  },
  { key: "date", label: "Date", value: (e) => e.date ?? "" },
];

export function matchesFilter(e: EntrySummary, filter: string) {
  const words = filter.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return true;
  const hay = [
    e.white,
    e.black,
    e.event,
    e.site,
    e.eco,
    e.result,
    e.date,
    e.round,
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
  return words.every((w) => hay.includes(w));
}

export function GameList({
  entries,
  selected,
  loading,
  onPreview,
  onOpen,
}: {
  entries: EntrySummary[];
  selected?: string;
  loading?: string;
  onPreview: (ordinal: string) => void;
  onOpen: (ordinal: string) => void;
}) {
  const body = useRef<HTMLTableSectionElement>(null);
  useEffect(() => {
    body.current
      ?.querySelector('[aria-selected="true"]')
      ?.scrollIntoView?.({ block: "nearest" });
  }, [selected]);
  function onKeyDown(event: KeyboardEvent) {
    const i = entries.findIndex((e) => e.ordinal === selected);
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const next =
        entries[
          Math.max(
            0,
            Math.min(
              entries.length - 1,
              i + (event.key === "ArrowDown" ? 1 : -1),
            ),
          )
        ];
      if (next) onPreview(next.ordinal);
    } else if (event.key === "Enter" && selected) {
      event.preventDefault();
      onOpen(selected);
    }
  }
  return (
    <div
      className="game-list"
      tabIndex={0}
      onKeyDown={onKeyDown}
      aria-label="Game list"
    >
      <table>
        <thead>
          <tr>
            <th className="num">#</th>
            {columns.map((c) => (
              <th key={c.key} className={c.className}>
                {c.label}
              </th>
            ))}
            <th>Status</th>
          </tr>
        </thead>
        <tbody ref={body}>
          {entries.slice(0, 200).map((e) => (
            <tr
              key={e.ordinal}
              aria-selected={selected === e.ordinal}
              data-deleted={e.deleted || undefined}
              data-kind={e.kind}
              onClick={() => onPreview(e.ordinal)}
              onDoubleClick={() => onOpen(e.ordinal)}
            >
              <td className="num">
                <button
                  className="entry-number"
                  onClick={(ev) => {
                    ev.stopPropagation();
                    onPreview(e.ordinal);
                  }}
                  aria-label={`Load entry ${e.ordinal}`}
                >
                  {e.ordinal}
                </button>
              </td>
              {columns.map((c) => (
                <td key={c.key} className={c.className}>
                  {c.value(e)}
                </td>
              ))}
              <td className="status">
                {e.deleted ? "Deleted " : ""}
                {loading === e.ordinal ? "Loading…" : ""}
                {e.kind === "unknown" ? "Unsupported entry" : ""}
              </td>
            </tr>
          ))}
          {entries.length === 0 && (
            <tr className="empty-row">
              <td colSpan={columns.length + 2}>No entries to show.</td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
