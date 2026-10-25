import { useCallback, useEffect, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type {
  SrsDeckSummary,
  SrsListing,
  SrsNewOrder,
  SrsOptions,
  SrsSkipped,
  SrsTrainee,
} from "../api/types";
import {
  clock,
  defaultOptions,
  formatSteps,
  parseSteps,
  validateOptions,
} from "../state/srs";
import { SrsStudy } from "./SrsStudy";

/** The open database list, for decks made from its games. */
export interface SrsDatabase {
  databaseId: string;
  games: number;
  selected: number;
  selection: { start: string; end: string }[];
}

const explain = (err: unknown) =>
  (err as { message?: string })?.message ?? String(err);
const baseName = (path: string) =>
  (path.split(/[\\/]/).at(-1) ?? path).replace(/\.pgn$/i, "");
const plural = (n: number, one: string, many: string) =>
  `${n.toLocaleString()} ${n === 1 ? one : many}`;

function skippedLines(skipped: SrsSkipped): string[] {
  const lines: string[] = [];
  if (skipped.games)
    lines.push(`${plural(skipped.games, "game", "games")} could not be read.`);
  if (skipped.cardsOverLimit)
    lines.push(
      `${plural(skipped.cardsOverLimit, "line", "lines")} over the limit of 20,000 cards ${skipped.cardsOverLimit === 1 ? "was" : "were"} left out.`,
    );
  if (skipped.gamesNotRead)
    lines.push(
      `${plural(skipped.gamesNotRead, "game", "games")} after the limit ${skipped.gamesNotRead === 1 ? "was" : "were"} not read.`,
    );
  return lines;
}

/** Whether `e` is the Escape key of a dialog. */
const escape = (e: React.KeyboardEvent, close: () => void) => {
  if (e.key === "Escape") {
    e.stopPropagation();
    close();
  }
};

type Scope = "all" | "selection";

function OptionFields({
  options,
  onChange,
}: {
  options: SrsOptions;
  onChange: (o: SrsOptions) => void;
}) {
  return (
    <>
      <label>
        Trainee{" "}
        <select
          aria-label="Trainee"
          value={options.trainee}
          onChange={(e) =>
            onChange({ ...options, trainee: e.target.value as SrsTrainee })
          }
        >
          <option value="sideToMove">Side to move</option>
          <option value="white">White</option>
          <option value="black">Black</option>
        </select>
      </label>
      <label className="mask-check">
        <input
          type="checkbox"
          checked={options.includeVariations}
          onChange={(e) =>
            onChange({ ...options, includeVariations: e.target.checked })
          }
        />
        Include variations
      </label>
      <label className="mask-check">
        <input
          type="checkbox"
          checked={options.showNewFirst}
          onChange={(e) =>
            onChange({ ...options, showNewFirst: e.target.checked })
          }
        />
        Show new lines with arrows first
      </label>
      <label>
        New card order{" "}
        <select
          aria-label="New card order"
          value={options.newOrder}
          onChange={(e) =>
            onChange({ ...options, newOrder: e.target.value as SrsNewOrder })
          }
        >
          <option value="source">Source order</option>
          <option value="random">Random</option>
        </select>
      </label>
    </>
  );
}

/** New deck: from a PGN file, or from the games of the open list. */
function CreateForm({
  database,
  source,
  busy,
  onCreate,
  onCancel,
}: {
  database?: SrsDatabase;
  source: { kind: "pgn"; path: string } | { kind: "list" };
  busy: boolean;
  onCreate: (name: string, options: SrsOptions, scope: Scope) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(
    source.kind === "pgn" ? baseName(source.path) : "Deck",
  );
  const [options, setOptions] = useState<SrsOptions>(defaultOptions);
  const [scope, setScope] = useState<Scope>(
    database && database.selected > 1 ? "selection" : "all",
  );
  const valid = name.trim() !== "";
  return (
    <form
      className="srs-form"
      aria-label="New deck"
      onSubmit={(e) => {
        e.preventDefault();
        if (valid && !busy) onCreate(name.trim(), options, scope);
      }}
      onKeyDown={(e) => escape(e, onCancel)}
    >
      <h3>New deck</h3>
      {source.kind === "pgn" ? (
        <p className="path" title={source.path}>
          {source.path}
        </p>
      ) : (
        database && (
          <fieldset className="mask-row" disabled={busy}>
            <legend className="mask-label">Source</legend>
            <label className="mask-check">
              <input
                type="radio"
                name="srs-scope"
                checked={scope === "all"}
                onChange={() => setScope("all")}
              />
              All games in the list ({database.games.toLocaleString()})
            </label>
            <label className="mask-check">
              <input
                type="radio"
                name="srs-scope"
                checked={scope === "selection"}
                disabled={database.selected === 0}
                onChange={() => setScope("selection")}
              />
              Selected games ({database.selected.toLocaleString()})
            </label>
          </fieldset>
        )
      )}
      <div className="mask-row">
        <label>
          Name{" "}
          <input
            aria-label="Name"
            value={name}
            disabled={busy}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <OptionFields options={options} onChange={setOptions} />
      </div>
      <div className="modal-actions">
        <button type="submit" disabled={!valid || busy}>
          Create deck
        </button>
        <button type="button" disabled={busy} onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}

/** The options of a deck: its name, the steps and limits. */
function OptionsDialog({
  name: initialName,
  options: initial,
  error,
  busy,
  onSave,
  onClose,
}: {
  name: string;
  options: SrsOptions;
  error?: string;
  busy: boolean;
  onSave: (name: string, options: SrsOptions) => void;
  onClose: () => void;
}) {
  const [name, setName] = useState(initialName);
  const [learning, setLearning] = useState(formatSteps(initial.learningSteps));
  const [relearning, setRelearning] = useState(
    formatSteps(initial.relearningSteps),
  );
  const [numbers, setNumbers] = useState({
    graduatingInterval: String(initial.graduatingInterval),
    easyInterval: String(initial.easyInterval),
    newPerDay: String(initial.newPerDay),
    reviewsPerDay: String(initial.reviewsPerDay),
    maxInterval: String(initial.maxInterval),
  });
  const [rest, setRest] = useState(initial);
  const learningSteps = parseSteps(learning);
  const relearningSteps = parseSteps(relearning);
  const candidate: SrsOptions = {
    ...rest,
    learningSteps: learningSteps ?? [],
    relearningSteps: relearningSteps ?? [],
    graduatingInterval: Number(numbers.graduatingInterval),
    easyInterval: Number(numbers.easyInterval),
    newPerDay: Number(numbers.newPerDay),
    reviewsPerDay: Number(numbers.reviewsPerDay),
    maxInterval: Number(numbers.maxInterval),
  };
  const problem =
    name.trim() === ""
      ? "Choose a name."
      : !learningSteps
        ? "Learning steps are minutes separated by commas, such as 1, 10."
        : !relearningSteps
          ? "Relearning steps are minutes separated by commas, such as 10."
          : validateOptions(candidate);
  const number = (
    label: string,
    key: keyof typeof numbers,
    step: number | "any" = 1,
  ) => (
    <label>
      {label}{" "}
      <input
        type="number"
        aria-label={label}
        min={0}
        step={step}
        value={numbers[key]}
        onChange={(e) => setNumbers({ ...numbers, [key]: e.target.value })}
      />
    </label>
  );
  return (
    <div className="modal-backdrop">
      <form
        role="dialog"
        aria-modal="true"
        aria-label="Deck options"
        className="modal srs-options"
        onSubmit={(e) => {
          e.preventDefault();
          if (!problem && !busy) onSave(name.trim(), candidate);
        }}
        onKeyDown={(e) => escape(e, onClose)}
      >
        <h2>Deck options</h2>
        <label>
          Name{" "}
          <input
            aria-label="Name"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <label>
          Learning steps (minutes){" "}
          <input
            aria-label="Learning steps (minutes)"
            value={learning}
            onChange={(e) => setLearning(e.target.value)}
          />
        </label>
        <label>
          Relearning steps (minutes){" "}
          <input
            aria-label="Relearning steps (minutes)"
            value={relearning}
            onChange={(e) => setRelearning(e.target.value)}
          />
        </label>
        {number("Graduating interval (days)", "graduatingInterval", "any")}
        {number("Easy interval (days)", "easyInterval", "any")}
        {number("New cards per day", "newPerDay")}
        {number("Reviews per day", "reviewsPerDay")}
        {number("Maximum interval (days)", "maxInterval", "any")}
        <OptionFields options={rest} onChange={setRest} />
        <p className="hint">
          Variations and the trainee colour apply the next time the deck is
          updated from its source.
        </p>
        {(problem || error) && (
          <p className="replay-error" role="alert">
            {problem ?? error}
          </p>
        )}
        <div className="modal-actions">
          <button type="submit" disabled={!!problem || busy}>
            Save
          </button>
          <button type="button" onClick={onClose}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}

/**
 * Spaced Repetition: decks of lines made from a PGN file or the games of a
 * list, studied day by day.
 */
export function SrsDecks({
  api,
  database,
  onClose,
}: {
  api: WorkbenchApi;
  database?: SrsDatabase;
  onClose: () => void;
}) {
  const [listing, setListing] = useState<SrsListing>();
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [creating, setCreating] = useState<
    { kind: "pgn"; path: string } | { kind: "list" }
  >();
  const [updating, setUpdating] = useState<SrsDeckSummary>();
  const [updateScope, setUpdateScope] = useState<Scope>("all");
  const [editing, setEditing] = useState<{
    deck: SrsDeckSummary;
    options: SrsOptions;
    error?: string;
  }>();
  const [studying, setStudying] = useState<SrsDeckSummary>();

  const refresh = useCallback(async () => {
    try {
      const { now, utcOffset } = clock();
      setListing(await api.srsDecks(now, utcOffset));
    } catch (e) {
      setError(explain(e));
    }
  }, [api]);
  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = async (work: () => Promise<void>) => {
    setBusy(true);
    setError(undefined);
    try {
      await work();
    } catch (e) {
      setError(explain(e));
    } finally {
      setBusy(false);
    }
  };
  const ranges = (scope: Scope) =>
    scope === "selection" && database
      ? database.selection
      : [{ start: "1", end: String((database?.games ?? 0) + 1) }];

  const startPgn = async () => {
    setError(undefined);
    try {
      const path = await api.choosePgn();
      if (typeof path === "string") setCreating({ kind: "pgn", path });
    } catch (e) {
      setError(explain(e));
    }
  };
  const create = (name: string, options: SrsOptions, scope: Scope) =>
    run(async () => {
      if (!creating) return;
      const { now, utcOffset } = clock();
      const made =
        creating.kind === "pgn"
          ? await api.srsCreateFromPgn(
              creating.path,
              name,
              options,
              now,
              utcOffset,
            )
          : await api.srsCreateFromRows(
              database!.databaseId,
              ranges(scope),
              name,
              options,
              now,
              utcOffset,
            );
      setCreating(undefined);
      setNotice([
        `Created “${made.deck.name}” with ${plural(made.deck.cards, "card", "cards")}.`,
        ...skippedLines(made.skipped),
      ]);
      await refresh();
    });
  const update = (deck: SrsDeckSummary, scope: Scope) =>
    run(async () => {
      const { now, utcOffset } = clock();
      const made =
        deck.sourceKind === "pgn"
          ? await api.srsUpdateFromPgn(deck.id, deck.source, now, utcOffset)
          : await api.srsUpdateFromRows(
              deck.id,
              database!.databaseId,
              ranges(scope),
              now,
              utcOffset,
            );
      setUpdating(undefined);
      setNotice([
        `Updated “${deck.name}”: ${made.added} added, ${made.kept} kept, ${made.suspended} suspended, ${made.restored} restored.`,
        ...skippedLines(made.skipped),
      ]);
      await refresh();
    });
  const remove = (deck: SrsDeckSummary) =>
    run(async () => {
      if (!(await api.confirmDeleteDeck(deck.name))) return;
      await api.srsDelete(deck.id);
      setNotice([`Deleted “${deck.name}”.`]);
      await refresh();
    });
  const options = (deck: SrsDeckSummary) =>
    run(async () => {
      const detail = await api.srsDeck(deck.id);
      setEditing({ deck, options: detail.options });
    });
  const save = (name: string, next: SrsOptions) =>
    run(async () => {
      if (!editing) return;
      try {
        await api.srsSetOptions(editing.deck.id, next);
        if (name !== editing.deck.name)
          await api.srsRename(editing.deck.id, name);
      } catch (e) {
        setEditing({ ...editing, error: explain(e) });
        return;
      }
      setEditing(undefined);
      await refresh();
    });

  return (
    <div className="modal-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Spaced Repetition"
        className="modal srs-decks"
        onKeyDown={(e) => escape(e, onClose)}
      >
        <h2>Spaced Repetition</h2>
        <p className="hint">
          Train the lines of a PGN file or a database list: you play one side
          and mistakes come back soon.
        </p>
        {error && (
          <p className="replay-error" role="alert">
            {error}
          </p>
        )}
        {notice.length > 0 && (
          <div className="srs-notice" role="status">
            {notice.map((line, i) => (
              <p key={i}>{line}</p>
            ))}
          </div>
        )}
        {listing && listing.decks.length === 0 && (
          <p>No decks yet. Make one from a PGN file or a database list.</p>
        )}
        {listing && listing.decks.length > 0 && (
          <div className="table-scroll">
            <table className="srs-table">
              <thead>
                <tr>
                  <th scope="col">Name</th>
                  <th scope="col">Cards</th>
                  <th scope="col">New</th>
                  <th scope="col">Learning</th>
                  <th scope="col">Due</th>
                  <th scope="col">Retention</th>
                  <th scope="col" aria-label="Actions" />
                </tr>
              </thead>
              <tbody>
                {listing.decks.map((deck) => (
                  <tr key={deck.id}>
                    <td title={deck.source}>{deck.name}</td>
                    <td className="num">{deck.cards}</td>
                    <td className="num">{deck.stats.new}</td>
                    <td className="num">{deck.stats.learning}</td>
                    <td className="num">{deck.stats.dueToday}</td>
                    <td className="num">
                      {deck.stats.retention === null
                        ? "–"
                        : `${Math.round(deck.stats.retention * 100)}%`}
                    </td>
                    <td className="srs-row-actions">
                      <button
                        type="button"
                        aria-label={`Study ${deck.name}`}
                        disabled={busy}
                        onClick={() => setStudying(deck)}
                      >
                        Study
                      </button>
                      <button
                        type="button"
                        aria-label={`Options for ${deck.name}`}
                        disabled={busy}
                        onClick={() => void options(deck)}
                      >
                        Options
                      </button>
                      <button
                        type="button"
                        aria-label={`Update ${deck.name} from source`}
                        disabled={busy}
                        onClick={() =>
                          deck.sourceKind === "pgn"
                            ? void update(deck, "all")
                            : setUpdating(deck)
                        }
                      >
                        Update from source
                      </button>
                      <button
                        type="button"
                        aria-label={`Delete ${deck.name}`}
                        disabled={busy}
                        onClick={() => void remove(deck)}
                      >
                        Delete
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        {updating && (
          <div className="srs-form" role="group" aria-label="Update deck">
            <h3>Update “{updating.name}”</h3>
            {database ? (
              <fieldset className="mask-row" disabled={busy}>
                <legend className="mask-label">Source</legend>
                <label className="mask-check">
                  <input
                    type="radio"
                    name="srs-update-scope"
                    checked={updateScope === "all"}
                    onChange={() => setUpdateScope("all")}
                  />
                  All games in the list ({database.games.toLocaleString()})
                </label>
                <label className="mask-check">
                  <input
                    type="radio"
                    name="srs-update-scope"
                    checked={updateScope === "selection"}
                    disabled={database.selected === 0}
                    onChange={() => setUpdateScope("selection")}
                  />
                  Selected games ({database.selected.toLocaleString()})
                </label>
              </fieldset>
            ) : (
              <p>
                Open the database that holds the games, then update the deck
                from its list.
              </p>
            )}
            <div className="modal-actions">
              <button
                type="button"
                disabled={!database || busy}
                onClick={() => void update(updating, updateScope)}
              >
                Update deck
              </button>
              <button type="button" onClick={() => setUpdating(undefined)}>
                Cancel
              </button>
            </div>
          </div>
        )}
        {listing && listing.unreadable.length > 0 && (
          <div className="srs-unreadable">
            <h3>Deck files that cannot be used</h3>
            <p>
              These files are listed, not changed. A file made by a newer
              version of the app is kept as it is.
            </p>
            <ul>
              {listing.unreadable.map((u) => (
                <li key={u.file}>
                  <code>{u.file}</code> — {u.reason}
                </li>
              ))}
            </ul>
          </div>
        )}
        {creating ? (
          <CreateForm
            database={database}
            source={creating}
            busy={busy}
            onCreate={(name, options, scope) =>
              void create(name, options, scope)
            }
            onCancel={() => setCreating(undefined)}
          />
        ) : (
          <div className="modal-actions">
            <button
              type="button"
              disabled={busy}
              onClick={() => void startPgn()}
            >
              New deck from PGN…
            </button>
            {database && database.games > 0 && (
              <button
                type="button"
                disabled={busy}
                onClick={() => setCreating({ kind: "list" })}
              >
                New deck from list
              </button>
            )}
            <button type="button" onClick={onClose}>
              Close
            </button>
          </div>
        )}
      </div>
      {editing && (
        <OptionsDialog
          name={editing.deck.name}
          options={editing.options}
          error={editing.error}
          busy={busy}
          onSave={(name, next) => void save(name, next)}
          onClose={() => setEditing(undefined)}
        />
      )}
      {studying && (
        <SrsStudy
          api={api}
          deckId={studying.id}
          deckName={studying.name}
          onClose={() => {
            setStudying(undefined);
            void refresh();
          }}
        />
      )}
    </div>
  );
}
