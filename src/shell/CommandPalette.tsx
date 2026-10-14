import { useEffect, useMemo, useRef, useState } from "react";
import { commandList, ribbons, type ViewId } from "../commands/catalogue";
import { format } from "../commands/keys";
import type { Bindings } from "../commands/bindings";

function locations(view: ViewId) {
  const where: Record<string, string> = {};
  for (const tab of ribbons[view])
    for (const group of tab.groups)
      for (const item of group.items)
        where[item.id] ??= `${tab.label} › ${group.label}`;
  return where;
}

export function CommandPalette({
  view,
  bindings,
  onClose,
}: {
  view: ViewId;
  bindings: Bindings;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.focus(), []);
  const where = useMemo(() => locations(view), [view]);
  const results = useMemo(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    return commandList
      .filter((c) => c.views.includes(view) && c.id !== "command-search")
      .map((c) => {
        const hay = `${c.label} ${c.tip} ${where[c.id] ?? ""}`.toLowerCase();
        const ok = words.every((w) => hay.includes(w));
        const score = c.label.toLowerCase().startsWith(words[0] ?? "") ? 0 : 1;
        const available = bindings[c.id] && bindings[c.id]!.enabled !== false;
        return { c, ok, score, available };
      })
      .filter((r) => r.ok)
      .sort(
        (a, b) =>
          Number(!a.available) - Number(!b.available) || a.score - b.score,
      )
      .slice(0, 50);
  }, [query, view, bindings, where]);
  useEffect(() => setIndex(0), [query]);
  function run(i: number) {
    const r = results[i];
    if (!r?.available) return;
    onClose();
    void bindings[r.c.id]!.run();
  }
  return (
    <div className="palette-backdrop" onMouseDown={onClose}>
      <div
        className="palette"
        role="dialog"
        aria-label="Search commands"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <input
          ref={input}
          aria-label="Command"
          placeholder="Search commands, e.g. flip, kibitzer, save"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
            else if (e.key === "ArrowDown") {
              e.preventDefault();
              setIndex((i) => Math.min(results.length - 1, i + 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setIndex((i) => Math.max(0, i - 1));
            } else if (e.key === "Enter") run(index);
          }}
        />
        <ul role="listbox" aria-label="Commands">
          {results.map((r, i) => {
            const Icon = r.c.icon;
            return (
              <li
                key={r.c.id}
                role="option"
                aria-selected={i === index}
                aria-disabled={!r.available}
                onMouseEnter={() => setIndex(i)}
                onClick={() => run(i)}
              >
                <Icon size={16} className="ribbon-icon" aria-hidden />
                <span className="palette-label">{r.c.label}</span>
                <span className="palette-where">
                  {r.available ? where[r.c.id] : "Not yet available"}
                </span>
                {r.c.keys && (
                  <kbd>
                    {[r.c.keys, ...(r.c.alsoKeys ?? [])].map(format).join(" ")}
                  </kbd>
                )}
              </li>
            );
          })}
          {results.length === 0 && (
            <li className="palette-empty">No matching commands</li>
          )}
        </ul>
      </div>
    </div>
  );
}
