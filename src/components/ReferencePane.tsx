import { useEffect, useState } from "react";
import { GameList } from "../views/GameList";
import type { ReferenceState } from "../state/positionPanes";

const sources = {
  tree: "from the opening tree",
  line: "followed from the opening tree",
  search: "by position search",
};

/**
 * Games of the open database that reach the current board (ChessBase's
 * reference search). It follows the board; opening a game selects the
 * position found.
 */
export function ReferencePane({
  reference,
  hasDatabase,
  hasLine,
  onOpen,
}: {
  reference: ReferenceState;
  hasDatabase: boolean;
  hasLine: boolean;
  onOpen: (ordinal: string, node: number) => void;
}) {
  const [cursor, setCursor] = useState<number>();
  const [top, setTop] = useState(0);
  const { response, page } = reference;
  const entries = page?.entries ?? [];
  useEffect(() => {
    setTop(0);
    setCursor(undefined);
  }, [page]);
  const node = (ordinal: string) =>
    entries.find((e) => e.ordinal === ordinal)?.foundNode ?? 0;
  let message: string | undefined;
  if (!hasDatabase) message = "Open a database to search it.";
  else if (!hasLine) message = "Select a game to search its positions.";
  else if (reference.error) message = reference.error;
  return (
    <div className="reference-pane" aria-label="Reference search">
      <div className="pane-header">
        <strong>Reference</strong>
        {response && (
          <span className="hint">
            {response.hits} games {sources[response.source]}
            {response.source === "search" && !response.indexed
              ? " (no search booster)"
              : ""}
            {response.truncated ? " · stopped at the limit" : ""}
            {entries.length < Number(response.hits)
              ? ` · first ${entries.length} shown`
              : ""}
          </span>
        )}
        {reference.loading && <span className="hint">Searching…</span>}
      </div>
      {message ? (
        <div className="empty-state">{message}</div>
      ) : (
        <GameList
          label="Reference games"
          rowCount={entries.length}
          version={page}
          row={(i) => entries[i]}
          want={() => {}}
          top={top}
          onTop={setTop}
          cursor={cursor}
          onPreview={setCursor}
          onOpen={(row) => {
            const ordinal = entries[row]?.ordinal;
            if (ordinal) onOpen(ordinal, node(ordinal));
          }}
        />
      )}
    </div>
  );
}
