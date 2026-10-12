import { useState } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import { commands, ribbons, type ViewId } from "../commands/catalogue";
import { format } from "../commands/keys";
import type { Bindings } from "../commands/bindings";

export function tooltip(id: string, bindings: Bindings) {
  const meta = commands[id];
  const binding = bindings[id];
  const keys = meta.keys ? ` (${format(meta.keys)})` : "";
  if (!binding) return `${meta.tip}${keys}\nNot yet available`;
  return `${meta.tip}${keys}`;
}

export function Ribbon({
  view,
  bindings,
}: {
  view: ViewId;
  bindings: Bindings;
}) {
  const tabs = ribbons[view];
  const [active, setActive] = useState<Record<ViewId, string>>({
    database: "Home",
    game: "Home",
  });
  const [collapsed, setCollapsed] = useState(false);
  const current = tabs.find((t) => t.label === active[view]) ?? tabs[0];
  return (
    <div className={collapsed ? "ribbon collapsed" : "ribbon"}>
      <div className="ribbon-tabs" role="tablist" aria-label="Ribbon">
        {tabs.map((tab) => (
          <button
            key={tab.label}
            role="tab"
            aria-selected={tab === current}
            className="ribbon-tab"
            onClick={() => {
              setActive((a) => ({ ...a, [view]: tab.label }));
              setCollapsed(false);
            }}
            onDoubleClick={() => setCollapsed((c) => !c)}
          >
            {tab.label}
          </button>
        ))}
        <button
          className="ribbon-collapse"
          aria-label={collapsed ? "Expand ribbon" : "Minimize ribbon"}
          title={collapsed ? "Expand ribbon" : "Minimize ribbon"}
          onClick={() => setCollapsed((c) => !c)}
        >
          {collapsed ? <ChevronDown size={14} /> : <ChevronUp size={14} />}
        </button>
      </div>
      {!collapsed && (
        <div
          className="ribbon-panel"
          role="tabpanel"
          aria-label={current.label}
        >
          {current.groups.map((group) => {
            const large = group.items.filter((i) => i.size !== "small");
            const small = group.items.filter((i) => i.size === "small");
            return (
              <section
                className="ribbon-group"
                key={group.label}
                aria-label={group.label}
              >
                <div className="ribbon-group-body">
                  {large.map((i) => (
                    <RibbonButton
                      key={i.id}
                      id={i.id}
                      bindings={bindings}
                      large
                    />
                  ))}
                  {small.length > 0 && (
                    <div className="ribbon-small-stack">
                      {small.map((i) => (
                        <RibbonButton
                          key={i.id}
                          id={i.id}
                          bindings={bindings}
                        />
                      ))}
                    </div>
                  )}
                </div>
                <div className="ribbon-group-label">{group.label}</div>
              </section>
            );
          })}
        </div>
      )}
    </div>
  );
}

function RibbonButton({
  id,
  bindings,
  large,
}: {
  id: string;
  bindings: Bindings;
  large?: boolean;
}) {
  const meta = commands[id];
  const binding = bindings[id];
  const Icon = meta.icon;
  const label = binding?.label ?? meta.label;
  return (
    <button
      className={large ? "ribbon-button large" : "ribbon-button small"}
      title={tooltip(id, bindings)}
      disabled={!binding || binding.enabled === false}
      data-unavailable={!binding || undefined}
      aria-pressed={binding?.pressed}
      aria-keyshortcuts={meta.keys}
      onClick={() => void binding?.run()}
    >
      <Icon
        className="ribbon-icon"
        size={large ? 28 : 16}
        strokeWidth={large ? 1.5 : 1.75}
        aria-hidden
      />
      <span>{label}</span>
    </button>
  );
}
