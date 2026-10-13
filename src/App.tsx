import { useEffect, useMemo, useState } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";
import { Database, LayoutGrid, Search, X } from "lucide-react";
import { backend, type WorkbenchApi } from "./api/client";
import { annotationsIncomplete } from "./api/types";
import { useWorkbench, type WorkbenchState } from "./state/workbench";
import { useEditing } from "./state/editing";
import { useAnalysis } from "./state/analysis";
import { CommentEditor } from "./components/CommentEditor";
import { UnsavedDialog } from "./components/UnsavedDialog";
import { SearchMask } from "./components/SearchMask";
import { EnginePanel } from "./components/EnginePanel";
import { NotationTree } from "./components/NotationTree";
import { BookPane } from "./components/BookPane";
import { ReferencePane } from "./components/ReferencePane";
import { usePositionPanes } from "./state/positionPanes";
import { Diagnostics } from "./components/Diagnostics";
import { GameList, matchesFilter } from "./views/GameList";
import { BoardPane, type Step } from "./views/BoardPane";
import { Ribbon } from "./shell/Ribbon";
import { CommandPalette } from "./shell/CommandPalette";
import { useShortcuts, type Bindings } from "./commands/bindings";
import type { ViewId } from "./commands/catalogue";
import { format } from "./commands/keys";
import "./styles/theme.css";
import "./styles/shell.css";
import "./styles/board.css";

export type Theme = "white" | "blue" | "gray" | "black";
type Layout = "standard" | "wide-list" | "big-board" | "big-engine";
const themeKey = "workbench.theme";
function storedTheme(): Theme {
  try {
    const t = localStorage.getItem(themeKey);
    if (t === "white" || t === "blue" || t === "gray" || t === "black")
      return t;
  } catch {
    // Storage may be unavailable; the default theme applies.
  }
  return "white";
}

export function App({
  api = backend,
  initialState,
  initialView = "database",
}: {
  api?: WorkbenchApi;
  initialState?: WorkbenchState;
  initialView?: ViewId;
}) {
  const w = useWorkbench(api, initialState);
  const { state } = w;
  const e = useEditing(w, api);
  const a = useAnalysis(api, state);
  const [view, setView] = useState<ViewId>(initialView);
  const [theme, setTheme] = useState<Theme>(storedTheme);
  const [layout, setLayout] = useState<Layout>("standard");
  const [palette, setPalette] = useState(false);
  const [filter, setFilter] = useState("");
  const [filterOpen, setFilterOpen] = useState(false);
  const [maskOpen, setMaskOpen] = useState(false);
  const [flipped, setFlipped] = useState(false);
  const [ordinal, setOrdinal] = useState("1");
  const [square, setSquare] = useState<string>();
  const [promotion, setPromotion] = useState<string[]>([]);
  const [commentFocus, setCommentFocus] = useState(0);
  // Notation tabs, and the book and reference panes outside them.
  const [notationTab, setNotationTab] = useState<
    "notation" | "book" | "reference"
  >("notation");
  const [extraBook, setExtraBook] = useState(false);
  const [extraReference, setExtraReference] = useState(false);

  const game = state.document?.game;
  const selected = state.position?.nodeId ?? 0;
  const inGame = view === "game";
  const panes = usePositionPanes(api, state, {
    book: inGame && (notationTab === "book" || extraBook),
    reference: inGame && (notationTab === "reference" || extraReference),
  });
  const title = game
    ? `${game.tags.White ?? "?"} — ${game.tags.Black ?? "?"}`
    : "Game board";

  useEffect(() => {
    setSquare(undefined);
    setPromotion([]);
  }, [state.position]);
  useEffect(() => {
    try {
      localStorage.setItem(themeKey, theme);
    } catch {
      // Theme persistence is a convenience only.
    }
    document.documentElement.dataset.theme = theme;
  }, [theme]);
  // A database named on the command line opens at start, optionally at an
  // entry; debug builds may also run one command before the snapshot.
  const [startupCommand, setStartupCommand] = useState<string>();
  useEffect(() => {
    let live = true;
    void (async () => {
      const startup = await api.startupRequest().catch(() => null);
      if (!live || !startup?.path) return;
      await w.openDatabase(startup.path);
      if (startup.entry) {
        await w.loadEntry(startup.entry);
        setOrdinal(startup.entry);
        setView("game");
        if (startup.node !== undefined) await w.selectNode(startup.node);
      }
      if (startup.command) setStartupCommand(startup.command);
      else await api.startupShown().catch(() => {});
    })();
    return () => {
      live = false;
    };
  }, [api]);
  // Closing the game returns to the list.
  useEffect(() => {
    if (!game && view === "game") setView("database");
  }, [game, view]);

  function clickSquare(to: string) {
    if (e.busy || !game) return;
    const moves =
      state.position?.legalUci.filter(
        (m) => m.slice(0, 2) === square && m.slice(2, 4) === to,
      ) ?? [];
    if (moves.length > 1) {
      setPromotion(moves);
      return;
    }
    if (moves.length === 1) {
      void e.edit({ kind: "appendMove", parent: selected, uci: moves[0] });
      setSquare(undefined);
      return;
    }
    setSquare(
      state.position?.legalUci.some((m) => m.slice(0, 2) === to)
        ? to
        : undefined,
    );
  }
  function navigate(step: Step) {
    if (!game) return;
    let id = selected;
    if (step === "first") id = 0;
    if (step === "previous") id = game.nodes[id]?.parent ?? 0;
    if (step === "next") id = game.nodes[id]?.children[0] ?? id;
    if (step === "last") {
      let count = 0;
      while (
        game.nodes[id]?.children[0] !== undefined &&
        count++ < game.nodes.length
      )
        id = game.nodes[id].children[0];
    }
    void w.selectNode(id);
  }
  // Arrow keys replay moves in both views, as in ChessBase.
  useEffect(() => {
    function key(event: KeyboardEvent) {
      if (e.question || promotion.length || palette || event.defaultPrevented)
        return;
      const target = event.target as HTMLElement;
      if (
        target.closest?.(
          'input,textarea,select,[contenteditable="true"],[role="separator"]',
        )
      )
        return;
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      const step = (
        {
          ArrowLeft: "previous",
          ArrowRight: "next",
          Home: "first",
          End: "last",
        } as const
      )[event.key as "ArrowLeft"];
      if (step && game) {
        event.preventDefault();
        navigate(step);
      }
    }
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  async function jump() {
    if (!/^[0-9]+$/.test(ordinal) || BigInt(ordinal) === 0n) {
      w.setDiagnostics([
        { code: "bounds", message: "Enter a positive decimal entry number" },
      ]);
      return;
    }
    await e.run(() => w.loadEntry(BigInt(ordinal).toString()));
  }
  // Games from a position search open at the position found.
  const foundNode = (n: string) =>
    state.page?.entries.find((x) => x.ordinal === n)?.foundNode ?? 0;
  const preview = (n: string) => void e.run(() => w.loadEntry(n, foundNode(n)));
  async function open(n: string) {
    if (state.document?.summary.ordinal !== n || !game)
      await e.run(() => w.loadEntry(n, foundNode(n)));
    setView("game");
  }
  function step(delta: 1 | -1) {
    const current = state.document?.summary.ordinal;
    if (!current || !state.database) return;
    const next = BigInt(current) + BigInt(delta);
    if (next < 1n || next > BigInt(state.database.entryCount)) return;
    void e.run(() => w.loadEntry(next.toString()));
  }
  function readPage(delta: 1 | -1) {
    if (delta > 0) {
      if (state.page?.next) void w.readPage(state.page.next);
      return;
    }
    void w.readPage(
      (BigInt(w.pageStart) > 200n ? BigInt(w.pageStart) - 200n : 1n).toString(),
    );
  }

  /** Plays a book move: the existing continuation, or a new move. */
  function playBookMove(uci: string) {
    if (!game || e.busy) return;
    const existing = game.nodes[selected]?.children.find(
      (c) => game.nodes[c]?.uci === uci,
    );
    if (existing !== undefined) void w.selectNode(existing);
    else void e.edit({ kind: "appendMove", parent: selected, uci });
  }
  const openReference = (ordinal: string, node: number) =>
    void e.run(() => w.loadEntry(ordinal, node));
  const bookPane = (
    <BookPane
      book={panes.book}
      hasDatabase={!!state.database}
      hasLine={!!panes.line}
      onPlay={playBookMove}
      onCreate={() => void w.createTree()}
      building={w.treeBuild && `Building the opening tree: ${w.treeBuild}`}
    />
  );
  const referencePane = (
    <ReferencePane
      reference={panes.reference}
      hasDatabase={!!state.database}
      hasLine={!!panes.line}
      onOpen={openReference}
    />
  );

  const busy = e.busy;
  const bindings: Bindings = {
    "open-database": { run: () => e.run(w.openDatabase) },
    "open-database-edit": { run: () => e.run(w.openDatabaseForEditing) },
    "new-database": { run: () => e.run(w.createDatabase) },
    "save-game": {
      // Without a database open for editing, Save exports PGN.
      run: () =>
        state.database?.writable ? e.saveToDatabase("save") : e.save(),
      enabled: !!game && !busy,
    },
    "replace-game": {
      run: () => e.saveToDatabase("replace"),
      enabled:
        !!game &&
        !busy &&
        !!state.database?.writable &&
        state.origin?.databaseId === state.database.databaseId,
    },
    "delete-game": {
      run: () => e.toggleDeleted(state.document?.summary.ordinal ?? ""),
      enabled:
        !!state.database?.writable &&
        state.origin?.databaseId === state.database.databaseId,
    },
    "open-pgn": { run: () => e.run(e.openPgn) },
    "open-board": { run: () => setView("game"), enabled: !!game },
    "database-window": { run: () => setView("database") },
    "filter-games": {
      run: () => setMaskOpen(true),
      enabled: !!state.database,
      pressed: !!state.search,
    },
    "quick-filter": {
      run: () => setFilterOpen((o) => !o),
      enabled: !!state.page,
      pressed: filterOpen,
    },
    "find-position-db": {
      run: () => {
        const fen = state.position?.fen;
        if (!fen) return;
        setView("database");
        return w.findPosition(fen);
      },
      enabled: !!state.position && !!state.database,
    },
    "open-book": {
      run: () => {
        setView("game");
        setNotationTab("book");
      },
      enabled: !!game,
      pressed: inGame && notationTab === "book",
    },
    "toggle-book-pane": {
      run: () => setExtraBook((v) => !v),
      pressed: extraBook,
    },
    "toggle-reference-pane": {
      run: () => setExtraReference((v) => !v),
      pressed: extraReference,
    },
    "find-reference": {
      run: () => {
        setView("game");
        setNotationTab("reference");
      },
      enabled: !!state.position && !!state.database,
    },
    "create-opening-tree": {
      run: () => w.createTree(),
      enabled: !!state.database && !w.treeBuild,
    },
    "delete-opening-tree": {
      run: () => w.deleteTree(),
      enabled: !!state.database?.tree && !w.treeBuild,
    },
    "create-search-booster": {
      run: () => w.createBooster(),
      enabled: !!state.database,
    },
    "delete-search-booster": {
      run: () => w.deleteBooster(),
      enabled: !!state.database?.booster,
    },
    "reset-filter": {
      run: () => w.clearSearch(),
      enabled: !!state.search,
    },
    "previous-page": {
      run: () => readPage(-1),
      enabled: !!state.database && w.pageStart !== "1",
    },
    "next-page": { run: () => readPage(1), enabled: !!state.page?.next },
    "save-pgn": { run: () => e.save(), enabled: !!game && !busy },
    "close-game": {
      run: () => e.run(e.closeDocument),
      enabled: !!game && !busy,
    },
    "copy-position": {
      run: () => navigator.clipboard?.writeText(state.position?.fen ?? ""),
      enabled: !!state.position,
    },
    "flip-board": { run: () => setFlipped((f) => !f), pressed: flipped },
    "add-kibitzer": { run: () => a.choose(), enabled: !a.busy },
    "remove-kibitzer": { run: () => a.close(), enabled: !!a.engine && !a.busy },
    "toggle-analysis": {
      run: () => (a.enabled ? a.stop() : a.start()),
      enabled: !!a.engine && !!state.document?.documentId && !a.busy,
      pressed: a.enabled,
      label: a.enabled ? "Stop Analysis" : "Infinite Analysis",
    },
    "load-previous-game": {
      run: () => step(-1),
      enabled:
        !!state.database &&
        !!state.document &&
        state.document.summary.ordinal !== "1",
    },
    "load-next-game": {
      run: () => step(1),
      enabled:
        !!state.database &&
        !!state.document &&
        BigInt(state.document.summary.ordinal) <
          BigInt(state.database.entryCount),
    },
    "text-after": {
      run: () => setCommentFocus((n) => n + 1),
      enabled: !!game?.nodes[selected],
    },
    "layout-standard": {
      run: () => setLayout("standard"),
      pressed: layout === "standard",
    },
    "layout-wide-list": {
      run: () => setLayout("wide-list"),
      pressed: layout === "wide-list",
    },
    "layout-board-only": {
      run: () => setLayout("big-board"),
      pressed: layout === "big-board",
    },
    "layout-tall-engine": {
      run: () => setLayout("big-engine"),
      pressed: layout === "big-engine",
    },
    "theme-white": { run: () => setTheme("white"), pressed: theme === "white" },
    "theme-blue": { run: () => setTheme("blue"), pressed: theme === "blue" },
    "theme-gray": { run: () => setTheme("gray"), pressed: theme === "gray" },
    "theme-black": { run: () => setTheme("black"), pressed: theme === "black" },
    "command-search": { run: () => setPalette(true) },
    "stop-process": {
      run: () => (w.treeBuild ? w.cancelTree() : a.stop()),
      enabled: a.enabled || !!w.treeBuild,
    },
  };
  useEffect(() => {
    if (!startupCommand) return;
    setStartupCommand(undefined);
    void (async () => {
      await bindings[startupCommand]?.run();
      // Let the result render before the picture is taken.
      await new Promise((r) => setTimeout(r, 500));
      await api.startupShown().catch(() => {});
    })();
  }, [startupCommand]);
  useShortcuts(
    view,
    bindings,
    !!e.question || palette || maskOpen || promotion.length > 0,
  );
  // "/" also opens command search when focus is not in a field.
  useEffect(() => {
    function slash(event: KeyboardEvent) {
      if (event.key !== "/" || palette) return;
      if ((event.target as HTMLElement).closest?.("input,textarea,select"))
        return;
      event.preventDefault();
      setPalette(true);
    }
    window.addEventListener("keydown", slash);
    return () => window.removeEventListener("keydown", slash);
  }, [palette]);

  const entries = useMemo(
    () => (state.page?.entries ?? []).filter((x) => matchesFilter(x, filter)),
    [state.page, filter],
  );
  const databaseName = state.database ? "Database" : "Databases";
  const engineStatus = a.engine
    ? `${a.engine.identity.name ?? "UCI engine"} · ${a.enabled ? "analysing" : "idle"}`
    : "No engine";

  const pgnSelect = e.collection && (
    <label className="pgn-select">
      PGN game
      <select
        aria-label="PGN game"
        value={
          state.document?.metadataStatus === "available" &&
          state.document?.annotationsStatus === "available"
            ? Number(state.document.summary.ordinal) - 1
            : ""
        }
        onChange={(event) => {
          const index = Number(event.target.value);
          void e.run(() => e.loadPgn(index));
        }}
      >
        <option value="" disabled>
          Select game
        </option>
        {e.collection.games.map((tags, i) => (
          <option key={i} value={i}>
            {i + 1}. {tags.White ?? "?"} — {tags.Black ?? "?"}
          </option>
        ))}
      </select>
    </label>
  );

  const notation = game ? (
    <NotationTree
      nodes={game.nodes}
      selection={selected}
      onSelect={(id) => void w.selectNode(id)}
      startFen={game.startFen}
    />
  ) : (
    <div className="empty-state">
      Select a game to replay its moves and variations.
    </div>
  );

  return (
    <div className="app" data-theme={theme}>
      <header className="title-bar">
        <div className="brand" aria-hidden>
          <LayoutGrid size={16} />
        </div>
        <nav className="workspace-tabs" role="tablist" aria-label="Workspace">
          <button
            role="tab"
            aria-selected={view === "database"}
            className="workspace-tab"
            onClick={() => setView("database")}
          >
            <Database size={14} aria-hidden />
            <span>{databaseName}</span>
            {state.database && <small>{state.database.entryCount}</small>}
          </button>
          {game && (
            <span
              className="workspace-tab-wrap"
              data-selected={view === "game"}
            >
              <button
                role="tab"
                aria-selected={view === "game"}
                className="workspace-tab"
                onClick={() => setView("game")}
              >
                <LayoutGrid size={14} aria-hidden />
                <span>{title}</span>
                {state.dirty && (
                  <span className="dirty-dot" title="Unsaved changes">
                    ●
                  </span>
                )}
              </button>
              <button
                className="tab-close"
                aria-label="Close game tab"
                title="Close game"
                disabled={busy}
                onClick={() => void e.run(e.closeDocument)}
              >
                <X size={12} />
              </button>
            </span>
          )}
        </nav>
        <button
          className="command-search"
          onClick={() => setPalette(true)}
          title="Search commands"
        >
          <Search size={14} aria-hidden />
          <span>Search commands</span>
          <kbd>/</kbd>
        </button>
      </header>
      <Ribbon view={view} bindings={bindings} />
      <main className="workspace">
        {view === "database" ? (
          <Group
            key={`db-${layout}`}
            orientation="horizontal"
            className="split"
          >
            <Panel
              id="list"
              defaultSize={layout === "wide-list" ? "75" : "58"}
              minSize="30"
            >
              <section className="pane list-pane" aria-label="Database">
                <div className="pane-header">
                  <strong>
                    {state.search
                      ? `${state.search.hits} of ${state.database?.entryCount} entries found`
                      : state.database
                        ? `${state.database.entryCount} entries`
                        : "No database open"}
                  </strong>
                  {state.search && (
                    <button
                      className="link-button"
                      onClick={() => void w.clearSearch()}
                    >
                      Reset filter
                    </button>
                  )}
                  {pgnSelect}
                  {state.dirty && (
                    <span className="badge">Unsaved changes</span>
                  )}
                </div>
                {(filterOpen || filter) && (
                  <div className="filter-bar">
                    <input
                      aria-label="Filter games"
                      placeholder="Filter this page: player, event, ECO, result…"
                      value={filter}
                      autoFocus
                      onChange={(ev) => setFilter(ev.target.value)}
                      onKeyDown={(ev) => {
                        if (ev.key === "Escape") {
                          setFilter("");
                          setFilterOpen(false);
                        }
                      }}
                    />
                    <span className="hint">
                      {entries.length} of {state.page?.entries.length ?? 0} on
                      this page
                    </span>
                  </div>
                )}
                <GameList
                  entries={entries}
                  selected={state.document?.summary.ordinal}
                  loading={w.loadingEntry}
                  onPreview={preview}
                  onOpen={(n) => void open(n)}
                />
                <div className="pager">
                  <button
                    disabled={!state.database || w.pageStart === "1"}
                    onClick={() => readPage(-1)}
                  >
                    Previous page
                  </button>
                  <button
                    disabled={!state.page?.next}
                    onClick={() => readPage(1)}
                  >
                    Next page
                  </button>
                  <label>
                    Entry number{" "}
                    <input
                      value={ordinal}
                      onChange={(ev) => setOrdinal(ev.target.value)}
                      onKeyDown={(ev) => ev.key === "Enter" && void jump()}
                      inputMode="numeric"
                    />
                  </label>
                  <button
                    onClick={() => void jump()}
                    disabled={!state.database}
                  >
                    Go to entry
                  </button>
                </div>
              </section>
            </Panel>
            <Separator className="splitter vertical" />
            <Panel id="preview" minSize="25">
              <section className="pane preview-pane" aria-label="Preview">
                <BoardPane
                  title={title}
                  position={state.position}
                  flipped={flipped}
                  hasGame={!!game}
                  navigate={navigate}
                  compact
                  node={game?.nodes[selected]}
                />
                <div className="preview-notation">{notation}</div>
                <Diagnostics items={w.diagnostics} />
                {game && (
                  <button className="open-game" onClick={() => setView("game")}>
                    Open game <kbd>{format("Enter")}</kbd>
                  </button>
                )}
              </section>
            </Panel>
          </Group>
        ) : (
          <Group
            key={`game-${layout}`}
            orientation="vertical"
            className="split"
          >
            <Panel
              id="top"
              defaultSize={layout === "big-engine" ? "55" : "74"}
              minSize="30"
            >
              <Group orientation="horizontal" className="split">
                <Panel
                  id="board"
                  defaultSize={layout === "big-board" ? "68" : "52"}
                  minSize="30"
                >
                  <section className="pane" aria-label="Board">
                    <BoardPane
                      title={title}
                      position={state.position}
                      flipped={flipped}
                      hasGame={!!game}
                      navigate={navigate}
                      node={game?.nodes[selected]}
                      selectedSquare={square}
                      onSquare={clickSquare}
                      promotion={promotion}
                      onPromote={(uci) => {
                        if (uci)
                          void e.edit({
                            kind: "appendMove",
                            parent: selected,
                            uci,
                          });
                        setPromotion([]);
                      }}
                    />
                  </section>
                </Panel>
                <Separator className="splitter vertical" />
                <Panel id="notation" minSize="20">
                  <section className="pane notation-pane" aria-label="Notation">
                    <div className="pane-header">
                      <div
                        className="pane-tabs"
                        role="tablist"
                        aria-label="Notation tabs"
                      >
                        {(
                          [
                            ["notation", "Notation"],
                            ["book", "Book"],
                            ["reference", "Reference"],
                          ] as const
                        ).map(([tab, label]) => (
                          <button
                            key={tab}
                            role="tab"
                            aria-selected={notationTab === tab}
                            onClick={() => setNotationTab(tab)}
                          >
                            {label}
                          </button>
                        ))}
                      </div>
                      {pgnSelect}
                      {state.dirty && (
                        <span className="badge">Unsaved changes</span>
                      )}
                    </div>
                    {notationTab === "book" && bookPane}
                    {notationTab === "reference" && referencePane}
                    {notationTab === "notation" && (
                      <div className="notation-scroll">{notation}</div>
                    )}
                    {notationTab === "notation" && game?.nodes[selected] && (
                      <CommentEditor
                        key={commentFocus ? `focus-${commentFocus}` : "editor"}
                        node={game.nodes[selected]}
                        disabled={busy}
                        edit={e.edit}
                        autoFocus={commentFocus > 0}
                      />
                    )}
                    <Diagnostics items={w.diagnostics} />
                    {annotationsIncomplete(
                      state.document?.annotationsStatus,
                    ) && (
                      <p className="compatibility-note">
                        Some original database annotations are not shown.
                      </p>
                    )}
                  </section>
                </Panel>
              </Group>
            </Panel>
            <Separator
              className="splitter horizontal"
              aria-label="Resize analysis pane"
            />
            <Panel id="engine" minSize="12">
              <Group orientation="horizontal" className="split">
                <Panel id="kibitzer" minSize="20">
                  <section className="pane analysis-pane" aria-label="Engine">
                    <EnginePanel a={a} state={state} />
                  </section>
                </Panel>
                {extraBook && (
                  <>
                    <Separator className="splitter vertical" />
                    <Panel id="extra-book" minSize="20">
                      <section className="pane" aria-label="Book pane">
                        {bookPane}
                      </section>
                    </Panel>
                  </>
                )}
                {extraReference && (
                  <>
                    <Separator className="splitter vertical" />
                    <Panel id="extra-reference" minSize="20">
                      <section className="pane" aria-label="Reference pane">
                        {referencePane}
                      </section>
                    </Panel>
                  </>
                )}
              </Group>
            </Panel>
          </Group>
        )}
      </main>
      <footer className="status-bar">
        <span className="app-status">{w.status}</span>
        <span>{game ? `${title} · move ${selected}` : ""}</span>
        <span>{engineStatus}</span>
      </footer>
      {palette && (
        <CommandPalette
          view={view}
          bindings={bindings}
          onClose={() => setPalette(false)}
        />
      )}
      <UnsavedDialog question={e.question} answer={e.answer} />
      {maskOpen && (
        <SearchMask
          initial={state.search?.filter}
          onClose={() => setMaskOpen(false)}
          onSearch={(filter) => {
            setMaskOpen(false);
            setView("database");
            void w.search(filter);
          }}
        />
      )}
    </div>
  );
}
