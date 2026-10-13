import type { EntrySummary, SortColumn } from "../api/types";
import type { ListSort } from "../state/workbench";
import { VirtualTable } from "./VirtualTable";

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

/** The game list: entries of the database, a search or a sort. */
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
  isSelected,
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
  /** Ctrl toggles a row, Shift extends to it. */
  onPreview: (row: number, mods?: { toggle: boolean; extend: boolean }) => void;
  onOpen: (row: number) => void;
  /** Rows of a multiple selection; without it, the chosen row is selected. */
  isSelected?: (row: number) => boolean;
  label?: string;
}) {
  const mods = (e: {
    ctrlKey: boolean;
    metaKey: boolean;
    shiftKey: boolean;
  }) => ({
    toggle: e.ctrlKey || e.metaKey,
    extend: e.shiftKey,
  });
  const ariaSort = (column?: SortColumn) =>
    sort?.column === column
      ? sort?.descending
        ? "descending"
        : "ascending"
      : column === undefined && !sort
        ? "ascending"
        : "none";
  const header = (
    <tr>
      <th className="num" aria-sort={onSort ? ariaSort(undefined) : undefined}>
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
  );
  const renderRow = (i: number) => {
    const e = row(i);
    if (!e)
      return (
        <tr key={i} aria-rowindex={i + 2} className="placeholder-row">
          <td colSpan={columns.length + 2} />
        </tr>
      );
    return (
      <tr
        key={i}
        aria-rowindex={i + 2}
        aria-selected={
          isSelected
            ? isSelected(i)
            : cursor === i || (cursor === undefined && selected === e.ordinal)
        }
        data-deleted={e.deleted || undefined}
        data-kind={e.kind}
        onClick={(ev) => onPreview(i, mods(ev))}
        onDoubleClick={() => onOpen(i)}
      >
        <td className="num">
          <button
            className="entry-number"
            tabIndex={-1}
            onClick={(ev) => {
              ev.stopPropagation();
              onPreview(i, mods(ev));
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
    );
  };
  return (
    <VirtualTable
      label={label}
      rowCount={rowCount}
      version={version}
      want={want}
      top={top}
      onTop={onTop}
      onMeasure={onMeasure}
      cursor={cursor}
      onChoose={onPreview}
      onOpen={onOpen}
      columns={[76, ...columns.map((c) => c.width), 96]}
      header={header}
      renderRow={renderRow}
    />
  );
}
