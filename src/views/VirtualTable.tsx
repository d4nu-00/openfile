import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
  type WheelEvent,
} from "react";

/** Height of a table row in pixels; the stylesheet uses the same. */
export const ROW_HEIGHT = 22;
/** Rows assumed visible before the table has been measured. */
const DEFAULT_VISIBLE = 30;
/** Rows read ahead on each side of the window. */
const AHEAD = 50;

/**
 * A table of `rowCount` rows that shows a window starting at `top` and
 * scrolls itself. A list of millions of rows is taller than a browser
 * element may be, so the table keeps its own first row and scrollbar; the
 * columns still scroll natively. Rows are drawn by `renderRow` and read
 * ahead through `want`.
 */
export function VirtualTable({
  label,
  rowCount,
  version,
  want,
  top,
  onTop,
  onMeasure,
  cursor,
  onChoose,
  onOpen,
  columns,
  header,
  renderRow,
  empty = "No entries to show.",
}: {
  label: string;
  rowCount: number;
  /** Changes when the rows change. */
  version: unknown;
  want: (from: number, to: number) => void;
  top: number;
  onTop: (top: number) => void;
  onMeasure?: (visible: number) => void;
  /** The chosen row; it is kept in view. */
  cursor?: number;
  /** Arrow keys, Home and End choose a row. */
  onChoose: (row: number) => void;
  /** Enter opens the chosen row. */
  onOpen: (row: number) => void;
  /** Column widths in pixels. */
  columns: number[];
  header: ReactNode;
  renderRow: (row: number) => ReactNode;
  empty?: string;
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
  useEffect(() => {
    want(first - AHEAD, first + visible + AHEAD);
  }, [first, visible, rowCount, version]);
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
    if ((event.target as HTMLElement).closest?.("input,select,textarea"))
      return;
    const at = cursor ?? first - 1;
    const move = (to: number) => {
      event.preventDefault();
      const target = Math.min(Math.max(0, to), rowCount - 1);
      if (target >= 0) onChoose(target);
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
  function onTrackDown(event: PointerEvent) {
    if (event.target !== event.currentTarget) return;
    const el = track.current;
    if (!el?.clientHeight) return;
    const rect = el.getBoundingClientRect();
    const above = event.clientY - rect.top < (thumbTop / 100) * rect.height;
    // A click beside the thumb moves a screen, as native bars do.
    scrollTo(above ? first - visible : first + visible);
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
  for (let i = first; i < Math.min(rowCount, first + visible); i++)
    shown.push(renderRow(i));
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
            {columns.map((w, i) => (
              <col key={i} style={{ width: w }} />
            ))}
          </colgroup>
          <thead ref={head}>{header}</thead>
          <tbody>
            {shown}
            {rowCount === 0 && (
              <tr className="empty-row">
                <td colSpan={columns.length}>{empty}</td>
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
