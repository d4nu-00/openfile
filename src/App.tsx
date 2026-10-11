import { useEffect, useState, type CSSProperties } from "react";
import { backend, type WorkbenchApi } from "./api/client";
import { useWorkbench, type WorkbenchState } from "./state/workbench";
import { Board } from "./components/Board";
import { DatabaseTable } from "./components/DatabaseTable";
import { NotationTree } from "./components/NotationTree";
import { Toolbar } from "./components/Toolbar";
import { Diagnostics } from "./components/Diagnostics";
import "./styles/layout.css";
import "./styles/board.css";
const initialFen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
export function App({
  api = backend,
  initialState,
}: {
  api?: WorkbenchApi;
  initialState?: WorkbenchState;
}) {
  const w = useWorkbench(api, initialState);
  const { state } = w;
  const [flipped, setFlipped] = useState(false);
  const [ordinal, setOrdinal] = useState("1");
  const [analysisHeight, setAnalysisHeight] = useState(180);
  const game = state.document?.game;
  const selected = state.position?.nodeId ?? 0;
  function navigate(action: "first" | "previous" | "next" | "last") {
    if (!game) return;
    let id = selected;
    if (action === "first") id = 0;
    if (action === "previous") id = game.nodes[id]?.parent ?? 0;
    if (action === "next") id = game.nodes[id]?.children[0] ?? id;
    if (action === "last") {
      let count = 0;
      while (
        game.nodes[id]?.children[0] !== undefined &&
        count++ < game.nodes.length
      )
        id = game.nodes[id].children[0];
    }
    void w.selectNode(id);
  }
  useEffect(() => {
    function key(e: KeyboardEvent) {
      const target = e.target as HTMLElement;
      if (
        target.closest?.(
          'input,textarea,select,[contenteditable="true"],[role="separator"]',
        )
      )
        return;
      const action = (
        {
          ArrowLeft: "previous",
          ArrowRight: "next",
          Home: "first",
          End: "last",
        } as const
      )[e.key as "ArrowLeft"];
      if (action && game) {
        e.preventDefault();
        navigate(action);
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
    await w.loadEntry(BigInt(ordinal).toString());
  }
  const resize = (height: number) =>
    setAnalysisHeight(Math.max(100, Math.min(450, height)));
  return (
    <main
      className="workbench"
      style={{ "--analysis-height": `${analysisHeight}px` } as CSSProperties}
    >
      <header className="app-header">
        <div className="brand">
          ♟ <strong>Chess Workbench</strong>
        </div>
        <button onClick={() => void w.openDatabase()}>Open database</button>
        <span className="app-status">{w.status}</span>
      </header>
      <section className="database">
        <div className="database-controls">
          <strong>Database</strong>
          <span>{state.database?.entryCount ?? "0"} entries</span>
          <button
            disabled={!state.database || w.pageStart === "1"}
            onClick={() =>
              void w.readPage(
                (BigInt(w.pageStart) > 200n
                  ? BigInt(w.pageStart) - 200n
                  : 1n
                ).toString(),
              )
            }
          >
            Previous page
          </button>
          <button
            disabled={!state.page?.next}
            onClick={() => state.page?.next && void w.readPage(state.page.next)}
          >
            Next page
          </button>
          <label>
            Entry number{" "}
            <input
              value={ordinal}
              onChange={(e) => setOrdinal(e.target.value)}
              inputMode="numeric"
            />
          </label>
          <button onClick={() => void jump()} disabled={!state.database}>
            Go to entry
          </button>
        </div>
        <DatabaseTable
          entries={state.page?.entries ?? []}
          onLoad={(n) => void w.loadEntry(n)}
          selected={state.document?.summary.ordinal}
          loading={w.loadingEntry}
        />
      </section>
      <section className="game-area">
        <div className="board-column">
          <h2>
            {game
              ? `${game.tags.White ?? "?"} — ${game.tags.Black ?? "?"}`
              : "Game board"}
          </h2>
          <Board
            fen={state.position?.fen ?? initialFen}
            flipped={flipped}
            legalUci={state.position?.legalUci ?? []}
          />
          <Toolbar
            navigate={navigate}
            flip={() => setFlipped((v) => !v)}
            enabled={!!game}
          />
          <p className="position-label">
            {state.position?.turn ?? "white"} to move
          </p>
        </div>
        <div className="notation-column">
          {game ? (
            <NotationTree
              nodes={game.nodes}
              selection={selected}
              onSelect={(id) => void w.selectNode(id)}
            />
          ) : (
            <div className="empty-state">
              Select a game to replay its moves and variations.
            </div>
          )}
          <Diagnostics items={w.diagnostics} />
          {state.document?.annotationsStatus === "notDecoded" && (
            <p className="compatibility-note">
              Original database annotations are not decoded.
            </p>
          )}
        </div>
      </section>
      <div
        role="separator"
        aria-label="Resize analysis pane"
        aria-orientation="horizontal"
        aria-valuemin={100}
        aria-valuemax={450}
        aria-valuenow={analysisHeight}
        tabIndex={0}
        className="pane-splitter"
        onKeyDown={(e) => {
          if (e.key === "ArrowUp" || e.key === "ArrowDown") {
            e.preventDefault();
            resize(analysisHeight + (e.key === "ArrowUp" ? 20 : -20));
          }
        }}
        onPointerDown={(e) => {
          const startY = e.clientY,
            height = analysisHeight;
          const move = (ev: PointerEvent) =>
            resize(height + startY - ev.clientY);
          const up = () => {
            window.removeEventListener("pointermove", move);
            window.removeEventListener("pointerup", up);
          };
          window.addEventListener("pointermove", move);
          window.addEventListener("pointerup", up);
        }}
      />
      <section className="analysis-pane" style={{ height: analysisHeight }}>
        <h3>Analysis</h3>
        <p>Engine analysis will appear here when a UCI engine is selected.</p>
      </section>
    </main>
  );
}
