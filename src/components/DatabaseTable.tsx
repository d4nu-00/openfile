import type { EntrySummary } from "../api/types";
export function DatabaseTable({
  entries,
  onLoad,
  selected,
  loading,
}: {
  entries: EntrySummary[];
  onLoad: (ordinal: string) => void;
  selected?: string;
  loading?: string;
}) {
  return (
    <div className="database-scroll">
      <table>
        <thead>
          <tr>
            <th>Entry</th>
            <th>White</th>
            <th>Black</th>
            <th>Result</th>
            <th>Event / date</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          {entries.slice(0, 200).map((e) => (
            <tr key={e.ordinal} aria-selected={selected === e.ordinal}>
              <td>
                <button
                  onClick={() => onLoad(e.ordinal)}
                  aria-label={`Load entry ${e.ordinal}`}
                >
                  {e.ordinal}
                </button>
              </td>
              <td>{e.kind === "game" ? (e.white ?? "?") : "Text entry"}</td>
              <td>{e.black ?? (e.kind === "game" ? "?" : "")}</td>
              <td>{e.result ?? ""}</td>
              <td>
                {e.event} {e.date}
              </td>
              <td>
                {e.deleted ? "Deleted " : ""}
                {loading === e.ordinal ? "Loading…" : ""}
                {e.kind === "unknown" ? "Unsupported entry" : ""}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
