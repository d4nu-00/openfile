import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent,
  type WheelEvent,
} from "react";
import type { EntrySummary, SortColumn } from "../api/types";
import type { ListSort } from "../state/workbench";
import { BLOCK } from "../state/gameList";

/** Height of a list row in pixels; the stylesheet uses the same. */
export const ROW_HEIGHT = 22;
/** Rows assumed visible before the list has been measured. */
const DEFAULT_VISIBLE = 30;

const columns: {
  key: string;
  label: string;
  sort?: SortColumn;
  width: number;
  className?: string;
  value: (e: EntrySummary) => string | number | undefined;
}[] = [
  {
    key: "white",
    label: "White",
    sort: "white",
    width: 160,
    value: (e) => (e.kind === "game" ? (e.white ?? "?") : "Text entry"),
  },
  {
    key: "whiteElo",
    label: "Elo W",
    sort: "whiteElo",
    width: 52,
    className: "num",
    value: (e) => e.whiteElo || "",
  },
  {
    key: "black",
    label: "Black",
    sort: "black",
    width: 160,
    value: (e) => e.black ?? (e.kind === "game" ? "?" : ""),
  },
  {
    key: "blackElo",
    label: "Elo B",
    sort: "blackElo",
    width: 52,
    className: "num",
    value: (e) => e.blackElo || "",
  },
  {
    key: "result",
    label: "Result",
    sort: "result",
    width: 58,
    className: "center",
    value: (e) => e.result ?? "",
  },
  {
    key: "eco",
    label: "ECO",
    sort: "eco",
    width: 46,
    value: (e) => e.eco ?? "",
  },
  {
    key: "event",
    label: "Tournament",
    sort: "tournament",
    width: 200,
    value: (e) => e.event ?? "",
  },
  {
    key: "round",
    label: "Round",
    sort: "round",
    width: 52,
    className: "center",
    value: (e) => e.round ?? "",
  },
  {
    key: "date",
    label: "Date",
    sort: "date",
    width: 86,
    value: (e) => e.date ?? "",
  },
  {
    key: "annotator",
    label: "Annotator",
    sort: "annotator",
    width: 120,
    value: (e) => e.annotator ?? "",
  },
  {
    key: "source",
    label: "Source",
    sort: "source",
    width: 120,
    value: (e) => e.source ?? "",
  },
];

/** The next sort after clicking `column`: ascending, descending, then none. */
export function nextSort(
  current: ListSort | undefined,
  column: SortColumn | undefined,
): ListSort | undefined {
  if (!column) return undefined;
  if (current?.column !== column) return { column, descending: false };
  return current.descending ? undefined : { column, descending: true };
}

/**
 * The game list. It shows a window of `rowCount` rows, starting at `top`,
 * and scrolls itself: a list of millions of rows is taller than a browser
 * element may be. Rows come from `row`, which loads them on `want`.
 */
export function GameList({
  rowCount,
  version,
  row,
  want,
  top,
  onTop,
  onMeasure,
  cursor,
  selected,
  loading,
  sort,
  sorting,
  onSort,
  onPreview,
  onOpen,
  label = "Game list",
}: {
  rowCount: number;
  /** Changes when rows arrive or the list changes. */
  version: unknown;
  row: (index: number) => EntrySummary | undefined;
  want: (from: number, to: number) => void;
  top: number;
  onTop: (top: number) => void;
  onMeasure?: (visible: number) => void;
  /** The row of the game chosen last. */
  cursor?: number;
  /** The entry number of the loaded game. */
  selected?: string;
  loading?: string;
  sort?: ListSort;
  sorting?: boolean;
  /** Without it, headers do not sort. */
  onSort?: (sort: ListSort | undefined) => void;
  onPreview: (row: number) => void;
  onOpen: (row: number) => void;
  label?: string;
}) {
  const viewport = useRef<HTMLDivElement>(null);
  const head = useRef<HTMLTableSectionElement>(null);
  const track = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(DEFAULT_VISIBLE);
  const wheel = useRef(0);
  const maxTop = Math.max(0, rowCount - visible);
  const first = Math.min(Math.max(0, top), maxTop);

  useLayoutEffect(() => {
    const el = viewport.current;
    if (!el) return;
    const measure = () => {
      const height = el.clientHeight - (head.current?.offsetHeight ?? 0);
      // Unmeasured (e.g. in tests): keep the default.
      const n = height > 0 ? Math.max(1, Math.floor(height / ROW_HEIGHT)) : 0;
      if (n) setVisible(n);
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  useEffect(() => onMeasure?.(visible), [visible]);
  // Rows around the window are read ahead.
  useEffect(() => {
    want(first - BLOCK / 2, first + visible + BLOCK / 2);
  }, [first, visible, rowCount, version]);
  // The chosen row stays in view.
  useEffect(() => {
    if (cursor === undefined) return;
    if (cursor < first) onTop(cursor);
    else if (cursor >= first + visible) onTop(cursor - visible + 1);
  }, [cursor]);

  const scrollTo = (t: number) =>
    onTop(Math.min(Math.max(0, Math.round(t)), maxTop));
  function onWheel(event: WheelEvent) {
    const lines =
      event.deltaMode === 1
        ? event.deltaY * 3
        : event.deltaMode === 2
          ? event.deltaY * visible
          : event.deltaY / ROW_HEIGHT;
    wheel.current += lines;
    const whole = Math.trunc(wheel.current);
    if (!whole) return;
    wheel.current -= whole;
    scrollTo(first + whole);
  }
  function onKeyDown(event: KeyboardEvent) {
    const at = cursor ?? first - 1;
    const move = (to: number) => {
      event.preventDefault();
      const target = Math.min(Math.max(0, to), rowCount - 1);
      if (target >= 0) onPreview(target);
    };
    switch (event.key) {
      case "ArrowDown":
        return move(at + 1);
      case "ArrowUp":
        return move(Math.max(at - 1, 0));
      case "Home":
        return move(0);
      case "End":
        return move(rowCount - 1);
      case "Enter":
        if (cursor !== undefined) {
          event.preventDefault();
          onOpen(cursor);
        }
    }
  }

  // Scrollbar: the thumb's place is the first row's share of the list.
  const thumbShare = rowCount ? Math.min(1, visible / rowCount) : 1;
  const thumbPercent = Math.max(thumbShare * 100, 4);
  const thumbTop = maxTop ? (first / maxTop) * (100 - thumbPercent) : 0;
  const drag = useRef<{ y: number; top: number } | undefined>(undefined);
  function trackTop(clientY: number) {
    const el = track.current;
    if (!el || !el.clientHeight) return first;
    const rect = el.getBoundingClientRect();
    const share = (clientY - rect.top) / rect.height;
    return share * rowCount - visible / 2;
  }
  function onTrackDown(event: PointerEvent) {
    if (event.target !== event.currentTarget) return;
    const target = trackTop(event.clientY);
    // A click above or below the thumb moves a screen, as native bars do.
    scrollTo(target < first ? first - visible : first + visible);
  }
  function onThumbDown(event: PointerEvent) {
    event.preventDefault();
    (event.target as HTMLElement).setPointerCapture?.(event.pointerId);
    drag.current = { y: event.clientY, top: first };
  }
  function onThumbMove(event: PointerEvent) {
    const start = drag.current;
    const el = track.current;
    if (!start || !el?.clientHeight) return;
    const free = el.clientHeight * (1 - thumbPercent / 100);
    if (free <= 0) return;
    scrollTo(start.top + ((event.clientY - start.y) / free) * maxTop);
  }

  const shown = [];
  for (let i = first; i < Math.min(rowCount, first + visible); i++) {
    const e = row(i);
    shown.push(
      e ? (
        <tr
          key={i}
          aria-rowindex={i + 2}
          aria-selected={
            cursor === i || (cursor === undefined && selected === e.ordinal)
          }
          data-deleted={e.deleted || undefined}
          data-kind={e.kind}
          onClick={() => onPreview(i)}
          onDoubleClick={() => onOpen(i)}
        >
          <td className="num">
            <button
              className="entry-number"
              tabIndex={-1}
              onClick={(ev) => {
                ev.stopPropagation();
                onPreview(i);
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
      ) : (
        <tr key={i} aria-rowindex={i + 2} className="placeholder-row">
          <td colSpan={columns.length + 2} />
        </tr>
      ),
    );
  }
  const ariaSort = (column?: SortColumn) =>
    sort?.column === column
      ? sort?.descending
        ? "descending"
        : "ascending"
      : column === undefined && !sort
        ? "ascending"
        : "none";
  return (
    <div
      className="game-list"
      tabIndex={0}
      onKeyDown={onKeyDown}
      onWheel={onWheel}
      aria-label={label}
    >
      <div className="list-viewport" ref={viewport}>
        <table aria-rowcount={rowCount + 1}>
          <colgroup>
            <col style={{ width: 76 }} />
            {columns.map((c) => (
              <col key={c.key} style={{ width: c.width }} />
            ))}
            <col style={{ width: 96 }} />
          </colgroup>
          <thead ref={head}>
            <tr>
              <th
                className="num"
                aria-sort={onSort ? ariaSort(undefined) : undefined}
              >
                {onSort ? (
                  <button
                    className="sort-header"
                    disabled={sorting}
                    onClick={() => onSort(undefined)}
                    title="Entry order"
                  >
                    #
                  </button>
                ) : (
                  "#"
                )}
              </th>
              {columns.map((c) => (
                <th
                  key={c.key}
                  className={c.className}
                  aria-sort={onSort ? ariaSort(c.sort) : undefined}
                >
                  {onSort ? (
                    <button
                      className="sort-header"
                      disabled={sorting}
                      aria-label={`Sort by ${c.label}`}
                      onClick={() => onSort(nextSort(sort, c.sort))}
                    >
                      {c.label}
                      {sort?.column === c.sort && (
                        <span className="sort-mark" aria-hidden>
                          {sort?.descending ? " ▼" : " ▲"}
                        </span>
                      )}
                    </button>
                  ) : (
                    c.label
                  )}
                </th>
              ))}
              <th>Status</th>
            </tr>
          </thead>
          <tbody>
            {shown}
            {rowCount === 0 && (
              <tr className="empty-row">
                <td colSpan={columns.length + 2}>No entries to show.</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <div
        className="list-scrollbar"
        ref={track}
        role="scrollbar"
        aria-label={`${label} position`}
        aria-orientation="vertical"
        aria-valuemin={1}
        aria-valuemax={Math.max(1, rowCount)}
        aria-valuenow={first + 1}
        onPointerDown={onTrackDown}
      >
        {rowCount > visible && (
          <div
            className="list-thumb"
            style={{ top: `${thumbTop}%`, height: `${thumbPercent}%` }}
            onPointerDown={onThumbDown}
            onPointerMove={onThumbMove}
            onPointerUp={() => (drag.current = undefined)}
            onPointerCancel={() => (drag.current = undefined)}
          />
        )}
      </div>
    </div>
  );
}
