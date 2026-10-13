import { useEffect, useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type { NameKind, NameRow } from "../api/types";
import { RowCache } from "../state/gameList";
import { VirtualTable } from "./VirtualTable";

const years = (r: NameRow) =>
  !r.firstYear
    ? ""
    : r.firstYear === r.lastYear
      ? String(r.firstYear)
      : `${r.firstYear}–${r.lastYear}`;

const layouts = {
  players: {
    label: "Players",
    find: "Find player (Last, First)",
    columns: [260, 70, 100],
    headers: ["Player", "Games", "Years"],
    cells: (r: NameRow) => [r.name ?? "?", r.games, years(r)],
  },
  tournaments: {
    label: "Tournaments",
    find: "Find tournament",
    columns: [240, 160, 70, 100],
    headers: ["Tournament", "Site", "Games", "Year"],
    cells: (r: NameRow) => [r.name ?? "?", r.site ?? "", r.games, years(r)],
  },
};

/**
 * The players or tournaments of the open database, alphabetically, with
 * their number of games (ChessBase's player and tournament lists). The list
 * is prepared once per database in the app's data folder.
 */
export function NamesPane({
  api,
  databaseId,
  kind,
  onChoose,
  onGames,
}: {
  api: WorkbenchApi;
  databaseId: string;
  kind: NameKind;
  /** The chosen name, for commands that act on it. */
  onChoose: (row: NameRow | undefined) => void;
  /** Shows the games of a name (double-click or Enter). */
  onGames: (row: NameRow) => void;
}) {
  const layout = layouts[kind];
  const [rowCount, setRowCount] = useState<number>();
  const [progress, setProgress] = useState<string>();
  const [error, setError] = useState<string>();
  const [top, setTop] = useState(0);
  const [cursor, setCursor] = useState<number>();
  const [query, setQuery] = useState("");
  const [, arrived] = useState(0);
  const rows = useRef(
    new RowCache<NameRow>(
      (db, start, limit, requestId) =>
        api.readNames(db, kind, start, limit, requestId).then((p) => p.rows),
      () => arrived((n) => n + 1),
      setError,
    ),
  ).current;
  rows.reset(`${databaseId}:${kind}:${rowCount ?? ""}`, databaseId);

  useEffect(() => {
    let live = true;
    setRowCount(undefined);
    setTop(0);
    setCursor(undefined);
    setError(undefined);
    onChoose(undefined);
    setProgress("preparing…");
    api
      .nameList(databaseId, kind, (p) => {
        const pct = Number((BigInt(p.done) * 100n) / BigInt(p.total || "1"));
        if (live) setProgress(`counting games, ${pct}%`);
      })
      .then((r) => {
        if (!live) return;
        if (r.rows === undefined) setError("Listing was cancelled");
        else setRowCount(Number(r.rows));
      })
      .catch((e: unknown) => {
        if (live) setError((e as { message?: string })?.message ?? String(e));
      })
      .finally(() => {
        if (live) setProgress(undefined);
      });
    return () => {
      live = false;
    };
  }, [api, databaseId, kind]);

  // Typing a name jumps to the first one at or after it.
  useEffect(() => {
    if (rowCount === undefined || !query.trim()) return;
    let live = true;
    const timer = setTimeout(() => {
      void api
        .findName(databaseId, kind, query)
        .then(async (row) => {
          if (!live) return;
          const at = Number(row) - 1;
          setTop(at);
          choose(at);
        })
        .catch(() => {});
    }, 200);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [query, rowCount]);

  function choose(row: number) {
    setCursor(row);
    void rows.entry(row, rowCount ?? 0).then((r) => onChoose(r));
  }
  function open(row: number) {
    void rows.entry(row, rowCount ?? 0).then((r) => r && onGames(r));
  }

  return (
    <div className="names-pane" aria-label={layout.label}>
      <div className="filter-bar">
        <input
          aria-label={layout.find}
          placeholder={layout.find}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <span className="hint">
          {progress
            ? `${layout.label}: ${progress}`
            : rowCount !== undefined
              ? `${rowCount.toLocaleString()} ${layout.label.toLowerCase()}`
              : ""}
          {progress && (
            <button
              className="link-button"
              onClick={() => void api.cancelNameList(databaseId)}
            >
              Cancel
            </button>
          )}
        </span>
      </div>
      {error && <div className="empty-state">{error}</div>}
      <VirtualTable
        label={`${layout.label} list`}
        rowCount={rowCount ?? 0}
        version={`${databaseId}:${kind}:${rowCount}`}
        want={(from, to) => rows.want(from, to, rowCount ?? 0)}
        top={top}
        onTop={setTop}
        cursor={cursor}
        onChoose={choose}
        onOpen={open}
        columns={layout.columns}
        empty={progress ? "Preparing the list…" : "No names to show."}
        header={
          <tr>
            {layout.headers.map((h) => (
              <th key={h} className={h === "Games" ? "num" : undefined}>
                {h}
              </th>
            ))}
          </tr>
        }
        renderRow={(i) => {
          const r = rows.row(i);
          if (!r)
            return (
              <tr key={i} className="placeholder-row">
                <td colSpan={layout.columns.length} />
              </tr>
            );
          const cells = layout.cells(r);
          return (
            <tr
              key={i}
              aria-selected={cursor === i}
              onClick={() => choose(i)}
              onDoubleClick={() => open(i)}
            >
              {cells.map((c, j) => (
                <td
                  key={j}
                  className={typeof c === "number" ? "num" : undefined}
                >
                  {j === 0 ? (
                    <button
                      className="entry-number name-button"
                      tabIndex={-1}
                      aria-label={`Choose ${c}`}
                      onClick={(ev) => {
                        ev.stopPropagation();
                        choose(i);
                      }}
                    >
                      {c}
                    </button>
                  ) : (
                    c
                  )}
                </td>
              ))}
            </tr>
          );
        }}
      />
    </div>
  );
}
