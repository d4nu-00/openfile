import { useEffect, useState, type CSSProperties } from "react";
import { backend, type WorkbenchApi } from "./api/client";
import { useWorkbench, type WorkbenchState } from "./state/workbench";
import { useEditing } from "./state/editing";
import { CommentEditor } from "./components/CommentEditor";
import { UnsavedDialog } from "./components/UnsavedDialog";
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
  const e = useEditing(w, api);
  const [square, setSquare] = useState<string>();
  const [promotion, setPromotion] = useState<string[]>([]);
  useEffect(() => {
    setSquare(undefined);
    setPromotion([]);
  }, [state.position]);
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
    function key(event: KeyboardEvent) {
      if (e.question || promotion.length) return;
      const target = event.target as HTMLElement;
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
      )[event.key as "ArrowLeft"];
      if (action && game) {
        event.preventDefault();
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
    await e.run(() => w.loadEntry(BigInt(ordinal).toString()));
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
        <button onClick={() => void e.run(w.openDatabase)}>
          Open database
        </button>
        <button onClick={() => void e.run(e.openPgn)}>Open PGN</button>
        <button disabled={!game || e.busy} onClick={() => void e.save()}>
          Save PGN
        </button>
        <button
          disabled={!game || e.busy}
          onClick={() => void e.run(e.closeDocument)}
        >
          Close game
        </button>
        {state.dirty && <span>Unsaved changes</span>}
        <span className="app-status">{w.status}</span>
      </header>
      <section className="database">
        <div className="database-controls">
          {e.collection && (
            <label>
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
          )}
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
          onLoad={(n) => void e.run(() => w.loadEntry(n))}
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
            selectedSquare={square}
            onSquare={clickSquare}
            legalUci={state.position?.legalUci ?? []}
          />
          {promotion.length > 0 && (
            <section role="dialog" aria-label="Choose promotion">
              {[
                ["q", "Queen"],
                ["r", "Rook"],
                ["b", "Bishop"],
                ["n", "Knight"],
              ].map(([code, label]) => (
                <button
                  key={code}
                  onClick={() => {
                    const uci = promotion.find((m) => m.endsWith(code));
                    if (uci)
                      void e.edit({
                        kind: "appendMove",
                        parent: selected,
                        uci,
                      });
                    setPromotion([]);
                  }}
                >
                  {label}
                </button>
              ))}
              <button onClick={() => setPromotion([])}>Cancel</button>
            </section>
          )}
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
          {game?.nodes[selected] && (
            <CommentEditor
              node={game.nodes[selected]}
              disabled={e.busy}
              edit={e.edit}
            />
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
      <UnsavedDialog question={e.question} answer={e.answer} />
    </main>
  );
}
