import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import {
  BookMarked,
  BookOpen,
  Briefcase,
  Cpu,
  Database,
  Flag,
  FolderOpen,
  FolderPlus,
  GraduationCap,
  Landmark,
  Library,
  Mail,
  Microscope,
  Newspaper,
  PenTool,
  Puzzle,
  Smile,
  Star,
  Swords,
  Trophy,
  User,
  X,
  Zap,
  type LucideIcon,
} from "lucide-react";
import type { WorkbenchApi } from "../api/client";
import type { DatabaseSymbol, StartScreenData } from "../api/types";

/** the reference app's database types (messages 50027–50046) and the workbench's
 * own icon for each. */
export const KINDS: Record<string, [string, LucideIcon]> = {
  unspecific: ["Unspecific", Database],
  work: ["Work", Briefcase],
  large: ["Large database", Library],
  openings: ["Openings", BookOpen],
  informant: ["Informant", Newspaper],
  classical: ["Classical Tournaments", Landmark],
  correspondence: ["Correspondence", Mail],
  tactics: ["Tactics", Zap],
  analysis: ["Analysis", Microscope],
  training: ["Training", GraduationCap],
  endings: ["Endings", Flag],
  studies: ["Studies", PenTool],
  problems: ["Problems", Puzzle],
  myGames: ["My games", User],
  computer: ["Computer chess", Cpu],
  magazine: ["Magazine/Express", BookMarked],
  patzer: ["Patzer", Smile],
  match: ["Match", Swords],
  recentTournaments: ["Recent Tournaments", Trophy],
  important: ["Important", Star],
};

export type StartGroup = "favorites" | "recent" | { folder: string };
export type StartView = import("../api/types").DatabaseView;

const sameGroup = (a: StartGroup, b: StartGroup) =>
  typeof a === "string" || typeof b === "string"
    ? a === b
    : a.folder === b.folder;

/**
 * The database start screen, after the reference app's database window: My
 * Databases, Recent Databases and folder shortcuts on the left, the chosen
 * group's database symbols on the right. Double-click opens a database.
 */
export function StartScreen({
  api,
  showFolders,
  view,
  onView,
  onOpen,
  onSelect,
  onProperties,
  refresh,
}: {
  api: WorkbenchApi;
  showFolders: boolean;
  view: StartView;
  onView: (view: StartView) => void;
  onOpen: (path: string) => void;
  /** The chosen symbol, for commands that act on it. */
  onSelect: (symbol: DatabaseSymbol | undefined) => void;
  onProperties: (symbol: DatabaseSymbol) => void;
  /** Changes when the lists may have changed. */
  refresh: number;
}) {
  const [data, setData] = useState<StartScreenData>();
  const [group, setGroup] = useState<StartGroup>("favorites");
  const [folder, setFolder] = useState<{
    symbols: DatabaseSymbol[];
    truncated: boolean;
    error?: string;
  }>();
  const [chosen, setChosen] = useState<string>();
  const [error, setError] = useState<string>();
  const [reload, setReload] = useState(0);
  const items = useRef(new Map<string, HTMLElement>());
  const focusPath = useRef<string | undefined>(undefined);
  const again = () => setReload((n) => n + 1);

  useEffect(() => {
    let live = true;
    api
      .startScreen()
      .then((d) => live && setData(d))
      .catch(
        (e: unknown) =>
          live && setError(String((e as { message?: string })?.message ?? e)),
      );
    return () => {
      live = false;
    };
  }, [api, refresh, reload]);

  useEffect(() => {
    if (typeof group === "string") {
      setFolder(undefined);
      return;
    }
    let live = true;
    setFolder({ symbols: [], truncated: false });
    api
      .folderDatabases(group.folder)
      .then((f) => live && setFolder(f))
      .catch(
        (e: unknown) =>
          live &&
          setFolder({
            symbols: [],
            truncated: false,
            error: String((e as { message?: string })?.message ?? e),
          }),
      );
    return () => {
      live = false;
    };
  }, [api, group, refresh, reload]);

  const symbols =
    group === "favorites"
      ? (data?.favorites ?? [])
      : group === "recent"
        ? (data?.recent ?? [])
        : (folder?.symbols ?? []);
  const selected = symbols.find((s) => s.path === chosen);
  useEffect(
    () => onSelect(selected),
    [
      selected?.path,
      selected?.favorite,
      selected?.kind,
      selected?.name,
      selected?.description,
    ],
  );
  useEffect(() => {
    const path =
      focusPath.current ??
      (document.activeElement === document.body ? chosen : undefined);
    if (path) items.current.get(path)?.focus();
    focusPath.current = undefined;
  }, [view]);
  const keyboard = (e: KeyboardEvent<HTMLElement>, s: DatabaseSymbol) => {
    const i = symbols.findIndex((x) => x.path === s.path);
    let next = i;
    if (e.key === "Home") next = 0;
    else if (e.key === "End") next = symbols.length - 1;
    else if (["ArrowRight", "ArrowDown"].includes(e.key))
      next = Math.min(symbols.length - 1, i + 1);
    else if (["ArrowLeft", "ArrowUp"].includes(e.key))
      next = Math.max(0, i - 1);
    else if (e.key === "Enter" && !e.altKey) {
      e.preventDefault();
      if (s.exists) onOpen(s.path);
      return;
    } else return;
    e.preventDefault();
    setChosen(symbols[next].path);
    items.current.get(symbols[next].path)?.focus();
  };
  const switchView = (next: StartView) => {
    if (
      document.activeElement &&
      [...items.current.values()].includes(
        document.activeElement as HTMLElement,
      )
    )
      focusPath.current = chosen;
    onView(next);
  };

  const act = (p: Promise<unknown>) =>
    void p
      .then(again)
      .catch((e: unknown) =>
        setError(String((e as { message?: string })?.message ?? e)),
      );
  const title =
    group === "favorites"
      ? "My Databases"
      : group === "recent"
        ? "Recent Databases"
        : (data?.folders.find((f) => f.path === group.folder)?.name ??
          "Folder");
  const empty = !data
    ? "Reading the lists…"
    : group === "favorites"
      ? "No databases yet. Open a database, or pin one from Recent Databases or a folder."
      : group === "recent"
        ? "No database opened yet."
        : (folder?.error ?? "No .cbh or .2cbh databases in this folder.");

  const icon = (s: DatabaseSymbol) => {
    const [label, Icon] = KINDS[s.kind] ?? KINDS.unspecific;
    return <Icon aria-label={label} />;
  };
  const size = (s: DatabaseSymbol) =>
    !s.exists
      ? "missing"
      : s.entries !== undefined
        ? `${s.entries.toLocaleString()} entries`
        : "unreadable";

  return (
    <div className="start-screen" aria-label="Databases start screen">
      <nav className="start-nav" aria-label="Database groups">
        {(
          [
            ["favorites", "My Databases", Star, data?.favorites.length],
            ["recent", "Recent Databases", Database, data?.recent.length],
          ] as const
        ).map(([id, label, Icon, n]) => (
          <button
            key={id}
            type="button"
            aria-pressed={sameGroup(group, id)}
            onClick={() => setGroup(id)}
          >
            <Icon aria-hidden />
            <span>{label}</span>
            {n ? <span className="count">{n}</span> : null}
          </button>
        ))}
        {showFolders && (
          <>
            <div className="start-nav-head">
              <span>Folders</span>
              <button
                type="button"
                className="icon-button"
                title="Add Folder Shortcut"
                aria-label="Add Folder Shortcut"
                onClick={() =>
                  void api.chooseFolder().then((path) => {
                    if (typeof path === "string")
                      act(
                        api
                          .addFolderShortcut(path)
                          .then(() => setGroup({ folder: path })),
                      );
                  })
                }
              >
                <FolderPlus aria-hidden />
              </button>
            </div>
            {data?.folders.map((f) => (
              <div key={f.path} className="start-folder">
                <button
                  type="button"
                  title={f.path}
                  aria-pressed={sameGroup(group, { folder: f.path })}
                  className={f.exists ? undefined : "missing"}
                  onClick={() => setGroup({ folder: f.path })}
                >
                  <FolderOpen aria-hidden />
                  <span>{f.name}</span>
                </button>
                <button
                  type="button"
                  className="icon-button"
                  aria-label={`Remove folder shortcut ${f.name}`}
                  title="Remove folder shortcut"
                  onClick={() => {
                    if (sameGroup(group, { folder: f.path }))
                      setGroup("favorites");
                    act(api.removeFolderShortcut(f.path));
                  }}
                >
                  <X aria-hidden />
                </button>
              </div>
            ))}
          </>
        )}
      </nav>
      <section className="start-main" aria-label={title}>
        <header className="start-head">
          <strong>{title}</strong>
          <div className="segmented" role="group" aria-label="View">
            {(
              [
                ["tile", "Tile"],
                ["details", "Details"],
                ["symbols", "Symbols"],
                ["smallSymbols", "Small Symbols"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                type="button"
                aria-pressed={view === id}
                onClick={() => switchView(id)}
              >
                {label}
              </button>
            ))}
          </div>
          {selected && (
            <div className="start-actions">
              <button type="button" onClick={() => onProperties(selected)}>
                Properties
              </button>
              <button
                type="button"
                onClick={() => onOpen(selected.path)}
                disabled={!selected.exists}
              >
                Open
              </button>
              {selected.favorite ? (
                <button
                  type="button"
                  onClick={() => act(api.unpinDatabase(selected.path))}
                >
                  Remove Database Symbol
                </button>
              ) : (
                <button
                  type="button"
                  onClick={() => act(api.pinDatabase(selected.path))}
                >
                  Pin to My Databases
                </button>
              )}
              {selected.favorite && (
                <label>
                  Type{" "}
                  <select
                    aria-label="Database type"
                    value={selected.kind}
                    onChange={(e) =>
                      act(api.setDatabaseKind(selected.path, e.target.value))
                    }
                  >
                    {(data?.kinds ?? Object.keys(KINDS)).map((k) => (
                      <option key={k} value={k}>
                        {KINDS[k]?.[0] ?? k}
                      </option>
                    ))}
                  </select>
                </label>
              )}
              {group === "recent" && (
                <button
                  type="button"
                  onClick={() => act(api.forgetRecent(selected.path))}
                >
                  Remove from Recent
                </button>
              )}
            </div>
          )}
        </header>
        {error && <p role="alert">{error}</p>}
        {folder?.truncated && (
          <p className="hint">
            Only the first 500 databases of this folder are shown.
          </p>
        )}
        {!symbols.length ? (
          <div className="empty-state">{empty}</div>
        ) : view !== "details" ? (
          <div
            className={`start-symbols start-${view}`}
            role="listbox"
            aria-label={`${title} symbols`}
          >
            {symbols.map((s) => (
              <button
                key={s.path}
                type="button"
                role="option"
                aria-selected={chosen === s.path}
                ref={(el) => {
                  if (el) items.current.set(s.path, el);
                  else items.current.delete(s.path);
                }}
                tabIndex={
                  chosen === s.path || (!selected && s === symbols[0]) ? 0 : -1
                }
                title={[s.description, s.path, s.problem]
                  .filter(Boolean)
                  .join("\n")}
                className={`db-symbol${s.exists ? "" : " missing"}`}
                onClick={() => setChosen(s.path)}
                onFocus={() => setChosen(s.path)}
                onDoubleClick={() => s.exists && onOpen(s.path)}
                onKeyDown={(e) => keyboard(e, s)}
              >
                {icon(s)}
                <span className="db-name">{s.name}</span>
                <span className="hint">
                  {size(s)}
                  {s.format ? ` · ${s.format === "new" ? "2CBH" : "CBH"}` : ""}
                </span>
                {view === "tile" && (
                  <span className="db-description">
                    {s.description || s.folder}
                  </span>
                )}
              </button>
            ))}
          </div>
        ) : (
          <table
            className="stat-table start-details"
            aria-label={`${title} details`}
          >
            <thead>
              <tr>
                <th>Name</th>
                <th className="num">Entries</th>
                <th>Format</th>
                <th>Type</th>
                <th>Folder</th>
                <th>Description</th>
              </tr>
            </thead>
            <tbody>
              {symbols.map((s) => (
                <tr
                  key={s.path}
                  aria-selected={chosen === s.path}
                  ref={(el) => {
                    if (el) items.current.set(s.path, el);
                    else items.current.delete(s.path);
                  }}
                  tabIndex={
                    chosen === s.path || (!selected && s === symbols[0])
                      ? 0
                      : -1
                  }
                  className={s.exists ? undefined : "missing"}
                  onClick={() => setChosen(s.path)}
                  onFocus={() => setChosen(s.path)}
                  onKeyDown={(e) => keyboard(e, s)}
                  onDoubleClick={() => s.exists && onOpen(s.path)}
                >
                  <td>
                    {icon(s)} {s.name}
                  </td>
                  <td className="num">
                    {s.entries?.toLocaleString() ??
                      (s.exists ? "?" : "missing")}
                  </td>
                  <td>{s.format === "new" ? "2CBH" : s.format ? "CBH" : ""}</td>
                  <td>{KINDS[s.kind]?.[0] ?? s.kind}</td>
                  <td title={s.folder}>{s.folder}</td>
                  <td>{s.description}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}
