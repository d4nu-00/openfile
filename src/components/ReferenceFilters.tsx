import { useEffect, useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type {
  NameRow,
  ReferenceFilter,
  ReferenceStatistics,
} from "../api/types";

const count = new Intl.NumberFormat();
const share = (part: number, total: number) =>
  total ? `${((part * 100) / total).toFixed(1)}%` : "—";

export function ReferenceFilters({
  api,
  databaseId,
  filter,
  onChange,
  statistics,
  loading,
  error,
}: {
  api: WorkbenchApi;
  databaseId: string;
  filter: ReferenceFilter;
  onChange: (next: ReferenceFilter) => void;
  statistics?: ReferenceStatistics;
  loading?: boolean;
  error?: string;
}) {
  const [query, setQuery] = useState("");
  const [choices, setChoices] = useState<NameRow[]>([]);
  const [namesError, setNamesError] = useState("");
  const [namesLoading, setNamesLoading] = useState(false);
  const ready = useRef<Promise<{ rows?: string }> | undefined>(undefined);
  useEffect(() => {
    ready.current = undefined;
    setQuery("");
    setChoices([]);
  }, [databaseId]);
  useEffect(() => {
    if (query.trim().length < 2 || filter.playerId !== undefined) {
      setChoices([]);
      return;
    }
    let active = true;
    const timer = setTimeout(async () => {
      try {
        if (!ready.current) {
          if (active) setNamesLoading(true);
          ready.current = api.nameList(databaseId, "players", () => {});
        }
        const list = await ready.current;
        if (!list.rows) throw new Error("Player search stopped");
        const start = await api.findName(databaseId, "players", query.trim());
        const page = await api.readNames(
          databaseId,
          "players",
          start,
          12,
          `reference-player-${query}`,
        );
        if (active) {
          setChoices(
            page.rows.filter((row) =>
              row.name
                ?.toLocaleLowerCase()
                .startsWith(query.trim().toLocaleLowerCase()),
            ),
          );
          setNamesError("");
          setNamesLoading(false);
        }
      } catch (e) {
        if (active) {
          ready.current = undefined;
          setNamesError((e as Error).message ?? String(e));
          setNamesLoading(false);
        }
      }
    }, 180);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [api, databaseId, query, filter.playerId]);
  const changeNumber = (
    key: "eloMin" | "eloMax" | "yearMin" | "yearMax",
    raw: string,
  ) => {
    const value = raw === "" ? undefined : Number(raw);
    if (
      value !== undefined &&
      (!Number.isInteger(value) || value < 1 || value > 9999)
    )
      return;
    onChange({ ...filter, [key]: value });
  };
  return (
    <div className="reference-analysis">
      <div className="reference-controls" aria-label="Reference filters">
        <label>
          Both Elo ≥{" "}
          <input
            aria-label="Minimum Elo for both players"
            type="number"
            min="1"
            max="9999"
            value={filter.eloMin ?? ""}
            onChange={(e) => changeNumber("eloMin", e.target.value)}
            placeholder="any"
          />
        </label>
        <label>
          ≤{" "}
          <input
            aria-label="Maximum Elo for both players"
            type="number"
            min="1"
            max="9999"
            value={filter.eloMax ?? ""}
            onChange={(e) => changeNumber("eloMax", e.target.value)}
            placeholder="any"
          />
        </label>
        <label>
          Year ≥{" "}
          <input
            aria-label="First year"
            type="number"
            min="1"
            max="9999"
            value={filter.yearMin ?? ""}
            onChange={(e) => changeNumber("yearMin", e.target.value)}
            placeholder="any"
          />
        </label>
        <label>
          ≤{" "}
          <input
            aria-label="Last year"
            type="number"
            min="1"
            max="9999"
            value={filter.yearMax ?? ""}
            onChange={(e) => changeNumber("yearMax", e.target.value)}
            placeholder="any"
          />
        </label>
        <label className="reference-check">
          <input
            type="checkbox"
            checked={filter.goodGames}
            onChange={(e) =>
              onChange({ ...filter, goodGames: e.target.checked })
            }
          />{" "}
          Good games
        </label>
      </div>
      <p className="reference-rule">
        Good games: both players Elo ≥ 2200, a known result, and at least 40
        half-moves. This is a metadata rule, not an engine quality judgement.
        Elo and year bounds exclude games missing those values.
      </p>
      <div className="reference-player">
        <label>
          Player{" "}
          <input
            aria-label="Find comparison player"
            value={
              filter.playerId === undefined
                ? query
                : (filter.playerName ?? query)
            }
            onChange={(e) => {
              setQuery(e.target.value);
              onChange({
                ...filter,
                playerId: undefined,
                playerName: undefined,
              });
            }}
            placeholder="Find a player"
          />
        </label>
        <label>
          <input
            type="radio"
            name="reference-colour"
            checked={!filter.playerBlack}
            onChange={() => onChange({ ...filter, playerBlack: false })}
          />{" "}
          White
        </label>
        <label>
          <input
            type="radio"
            name="reference-colour"
            checked={filter.playerBlack}
            onChange={() => onChange({ ...filter, playerBlack: true })}
          />{" "}
          Black
        </label>
        {filter.playerId !== undefined && (
          <button
            type="button"
            className="link-button"
            onClick={() => {
              setQuery("");
              onChange({
                ...filter,
                playerId: undefined,
                playerName: undefined,
              });
            }}
          >
            Clear player
          </button>
        )}
      </div>
      {choices.length > 0 && (
        <div
          className="reference-player-choices"
          role="listbox"
          aria-label="Players"
        >
          {choices.map((row) => (
            <button
              type="button"
              role="option"
              aria-selected="false"
              key={row.id}
              onClick={() => {
                setQuery(row.name ?? "?");
                setChoices([]);
                onChange({ ...filter, playerId: row.id, playerName: row.name });
              }}
            >
              {row.name} · {count.format(row.games)} games
            </button>
          ))}
        </div>
      )}
      {namesLoading && (
        <button
          type="button"
          className="link-button"
          onClick={() => {
            void api.cancelNameList(databaseId);
            setNamesLoading(false);
          }}
        >
          Stop player search
        </button>
      )}
      {namesError && (
        <p className="reference-rule" role="alert">
          Player list: {namesError}
        </p>
      )}
      {loading && <p className="reference-rule">Updating move choices…</p>}
      {error && (
        <p className="reference-rule" role="alert">
          {error}
        </p>
      )}
      {statistics && (
        <>
          <div className="reference-totals">
            <span>Full {count.format(statistics.fullGames)}</span>
            <span>
              Filtered {count.format(statistics.filteredGames)} /{" "}
              {count.format(statistics.scannedGames)} scanned
            </span>
            {filter.playerId !== undefined && (
              <span>
                {filter.playerName || query || "Player"} as{" "}
                {filter.playerBlack ? "Black" : "White"}:{" "}
                {count.format(statistics.playerGames)}
              </span>
            )}
          </div>
          {(filter.eloMin !== undefined ||
            filter.eloMax !== undefined ||
            filter.yearMin !== undefined ||
            filter.yearMax !== undefined ||
            filter.goodGames) && (
            <div className="reference-funnel" aria-label="Filter effects">
              {(filter.eloMin !== undefined || filter.eloMax !== undefined) && (
                <span>
                  Elo kept {count.format(statistics.afterElo)} /{" "}
                  {count.format(statistics.scannedGames)}
                </span>
              )}
              {(filter.yearMin !== undefined ||
                filter.yearMax !== undefined) && (
                <span>
                  Year kept {count.format(statistics.afterYear)} /{" "}
                  {count.format(statistics.afterElo)}
                </span>
              )}
              {filter.goodGames && (
                <span>
                  Good games kept {count.format(statistics.filteredGames)} /{" "}
                  {count.format(statistics.afterYear)}
                </span>
              )}
            </div>
          )}
          <p className="reference-rule">
            {count.format(statistics.missingElo)} missing a rating ·{" "}
            {count.format(statistics.missingYear)} missing a year among scanned
            games.
            {!statistics.complete &&
              " Filtered and player counts use the first indexed 100,000 games; percentages describe that sample."}
            {!statistics.fullComplete &&
              " Full counts also describe the position-search limit."}
          </p>
          <div
            className="reference-moves game-list"
            aria-label="Reference move comparison"
          >
            <table>
              <thead>
                <tr>
                  <th>Move</th>
                  <th>Full N · share</th>
                  <th>Filtered N · share</th>
                  {filter.playerId !== undefined && <th>Player N · share</th>}
                </tr>
              </thead>
              <tbody>
                {statistics.moves.map((move) => (
                  <tr key={move.san}>
                    <td className="book-move">{move.san}</td>
                    <td>
                      {count.format(move.full)} ·{" "}
                      {share(move.full, statistics.fullGames)}
                    </td>
                    <td>
                      {count.format(move.filtered)} ·{" "}
                      {share(move.filtered, statistics.filteredGames)}
                    </td>
                    {filter.playerId !== undefined && (
                      <td>
                        {count.format(move.player)} ·{" "}
                        {share(move.player, statistics.playerGames)}
                      </td>
                    )}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </div>
  );
}
