import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type {
  SrsDeckSummary,
  SrsPreview,
  SrsQueueResponse,
  SrsStudyCard,
} from "../api/types";
import { defaultOptions } from "../state/srs";
import { renderWorkbench } from "./harness";

const stats = {
  total: 40,
  new: 12,
  learning: 3,
  dueToday: 7,
  suspended: 0,
  mature: 5,
  reviewsToday: 2,
  retention: 0.93,
  forecast: [7, 1, 0, 0, 2, 0, 0],
};
const deck: SrsDeckSummary = {
  id: "d1",
  name: "Mate",
  source: "/synthetic/Mate.pgn",
  sourceKind: "pgn",
  cards: 40,
  stats,
};
const listDeck: SrsDeckSummary = {
  ...deck,
  id: "d2",
  name: "From list",
  source: "Games",
  sourceKind: "list",
  cards: 5,
  stats: { ...stats, retention: null },
};

const startFen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const afterE4 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";
const afterE5 = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2";
const minute = 60_000;
const previews: SrsPreview[] = [
  { grade: "again", dueInMs: minute, intervalDays: 0, kind: "learning" },
  { grade: "hard", dueInMs: 6 * minute, intervalDays: 0, kind: "learning" },
  { grade: "good", dueInMs: 86_400_000, intervalDays: 1, kind: "review" },
  { grade: "easy", dueInMs: 4 * 86_400_000, intervalDays: 4, kind: "review" },
];
const newState = {
  kind: "new" as const,
  step: 0,
  due: 0,
  interval: 0,
  ease: 2.5,
  reps: 0,
  lapses: 0,
};

/** 1. e4 (the opponent) e5 (the trainee, Black); another line plays d5. */
function study(extra: Partial<SrsStudyCard> = {}): SrsStudyCard {
  return {
    card: {
      id: "c1",
      startFen,
      moves: [
        { uci: "e2e4", san: "e4", comment: null },
        { uci: "e7e5", san: "e5", comment: "Symmetry" },
      ],
      trainee: "black",
      white: "Morphy",
      black: "Anderssen",
      event: "Paris",
      date: "1858.01.01",
      sourceLabel: "Mate.pgn · game 7",
      order: 0,
      suspended: false,
      removed: false,
    },
    learn: false,
    state: { ...newState, kind: "review", interval: 5 },
    plies: [
      { fen: startFen, legalUci: [], otherMoves: [] },
      {
        fen: afterE4,
        legalUci: ["e7e5", "d7d5", "c7c5", "g8f6"],
        otherMoves: ["d7d5"],
      },
    ],
    endFen: afterE5,
    previews,
    ...extra,
  };
}
function queueOf(
  next: SrsStudyCard | null,
  counts = { newDue: 0, learningDue: 1, reviewDue: 2 },
): SrsQueueResponse {
  return {
    queue: {
      cards: next ? [next.card.id] : [],
      newLeft: 20,
      reviewsLeft: 200,
      nextLearningDue: null,
      ahead: false,
      ...counts,
    },
    next,
    stats,
  };
}

async function openDecks(
  decks: SrsDeckSummary[] = [deck],
  unreadable: { file: string; reason: string }[] = [],
) {
  const ui = renderWorkbench({ initialView: "database" });
  vi.mocked(ui.api.srsDecks).mockResolvedValue({ decks, unreadable });
  await ui.user.click(
    await screen.findByRole("button", { name: "Load entry 1" }),
  );
  await ui.user.click(
    screen.getByRole("button", { name: "Spaced Repetition" }),
  );
  const dialog = await screen.findByRole("dialog", {
    name: "Spaced Repetition",
  });
  return { ui, dialog };
}

async function openStudy(first: SrsStudyCard = study()) {
  const { ui, dialog } = await openDecks();
  vi.mocked(ui.api.srsQueue).mockResolvedValue(queueOf(first));
  await within(dialog).findByText("Mate");
  await ui.user.click(
    within(dialog).getByRole("button", { name: "Study Mate" }),
  );
  const session = await screen.findByRole("dialog", { name: "Study Mate" });
  return { ui, session };
}

const square = (within_: HTMLElement, name: string) =>
  within(within_).getByRole("button", { name: new RegExp(`^${name} `) });

/** Plays the trainee's move after the opponent's first move is on the board. */
async function play(
  ui: Awaited<ReturnType<typeof openStudy>>["ui"],
  session: HTMLElement,
  from: string,
  to: string,
) {
  await ui.user.click(square(session, from));
  await ui.user.click(square(session, to));
}

async function opponentPlayed(session: HTMLElement) {
  await within(session).findByRole(
    "button",
    { name: "e4 White pawn" },
    { timeout: 3000 },
  );
}

const answerOf = (grade: string, mistakes: number, hints: number) => [
  "d1",
  "c1",
  grade,
  mistakes,
  hints,
  expect.any(Number),
  expect.any(Number),
  expect.any(Number),
];

describe("the Decks dialog", () => {
  it("opens from the database ribbon and tells that there are no decks", async () => {
    const { dialog } = await openDecks([]);
    expect(await within(dialog).findByText(/No decks yet/)).toBeVisible();
    expect(
      within(dialog).getByRole("button", { name: "New deck from PGN…" }),
    ).toBeVisible();
  });

  it("asks the backend for the decks with the time and the offset", async () => {
    const { ui } = await openDecks();
    await waitFor(() => expect(ui.api.srsDecks).toHaveBeenCalled());
    const [now, offset] = vi.mocked(ui.api.srsDecks).mock.calls[0];
    expect(Math.abs(now - Date.now())).toBeLessThan(60_000);
    expect(offset).toBe(-new Date().getTimezoneOffset());
  });

  it("shows the counts of each deck", async () => {
    const { dialog } = await openDecks([deck, listDeck]);
    const row = await within(dialog).findByRole("row", { name: /Mate/ });
    expect(
      within(row)
        .getAllByRole("cell")
        .slice(0, 6)
        .map((c) => c.textContent),
    ).toEqual(["Mate", "40", "12", "3", "7", "93%"]);
    const other = within(dialog).getByRole("row", { name: /From list/ });
    expect(within(other).getAllByRole("cell")[5]).toHaveTextContent("–");
    expect(
      within(dialog)
        .getAllByRole("columnheader")
        .map((h) => h.textContent),
    ).toEqual(["Name", "Cards", "New", "Learning", "Due", "Retention", ""]);
  });

  it("lists the deck files that cannot be read and says they are left alone", async () => {
    const { dialog } = await openDecks(
      [deck],
      [{ file: "broken.json", reason: "The deck file cannot be read" }],
    );
    expect(await within(dialog).findByText(/broken\.json/)).toBeVisible();
    expect(within(dialog).getByText(/not changed/i)).toBeVisible();
  });

  it("creates a deck from a PGN with the chosen options", async () => {
    const { ui, dialog } = await openDecks([]);
    vi.mocked(ui.api.srsCreateFromPgn).mockResolvedValue({
      deck,
      skipped: { games: 2, cardsOverLimit: 0, gamesNotRead: 0 },
    });
    await ui.user.click(
      within(dialog).getByRole("button", { name: "New deck from PGN…" }),
    );
    expect(ui.api.choosePgn).toHaveBeenCalled();
    const name = await within(dialog).findByLabelText("Name");
    expect(name).toHaveValue("two");
    await ui.user.clear(name);
    await ui.user.type(name, "Mate");
    await ui.user.selectOptions(within(dialog).getByLabelText("Trainee"), [
      "White",
    ]);
    await ui.user.click(within(dialog).getByLabelText("Include variations"));
    await ui.user.selectOptions(
      within(dialog).getByLabelText("New card order"),
      ["Random"],
    );
    await ui.user.click(
      within(dialog).getByLabelText("Show new lines with arrows first"),
    );
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Create deck" }),
    );
    await waitFor(() =>
      expect(ui.api.srsCreateFromPgn).toHaveBeenCalledWith(
        "/synthetic/two.pgn",
        "Mate",
        {
          ...defaultOptions,
          trainee: "white",
          includeVariations: false,
          newOrder: "random",
          showNewFirst: true,
        },
        expect.any(Number),
        expect.any(Number),
      ),
    );
    expect(await within(dialog).findByText(/Created “Mate”/)).toBeVisible();
    expect(within(dialog).getByText(/2 games could not be read/)).toBeVisible();
  });

  it("does not create a deck when no file is chosen or the name is empty", async () => {
    const { ui, dialog } = await openDecks([]);
    vi.mocked(ui.api.choosePgn).mockResolvedValueOnce(null);
    await ui.user.click(
      within(dialog).getByRole("button", { name: "New deck from PGN…" }),
    );
    expect(within(dialog).queryByLabelText("Name")).toBeNull();
    await ui.user.click(
      within(dialog).getByRole("button", { name: "New deck from PGN…" }),
    );
    const name = await within(dialog).findByLabelText("Name");
    await ui.user.clear(name);
    expect(
      within(dialog).getByRole("button", { name: "Create deck" }),
    ).toBeDisabled();
  });

  it("creates a deck from all the games of the list", async () => {
    const { ui, dialog } = await openDecks([]);
    vi.mocked(ui.api.srsCreateFromRows).mockResolvedValue({
      deck: listDeck,
      skipped: { games: 0, cardsOverLimit: 0, gamesNotRead: 0 },
    });
    await ui.user.click(
      within(dialog).getByRole("button", { name: "New deck from list" }),
    );
    expect(
      within(dialog).getByLabelText("All games in the list (401)"),
    ).toBeChecked();
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Create deck" }),
    );
    await waitFor(() =>
      expect(ui.api.srsCreateFromRows).toHaveBeenCalledWith(
        "db-1",
        [{ start: "1", end: "402" }],
        "Deck",
        defaultOptions,
        expect.any(Number),
        expect.any(Number),
      ),
    );
  });

  it("creates a deck from the selected games of the list", async () => {
    const { ui, dialog } = await openDecks([]);
    vi.mocked(ui.api.srsCreateFromRows).mockResolvedValue({
      deck: listDeck,
      skipped: { games: 0, cardsOverLimit: 0, gamesNotRead: 0 },
    });
    await ui.user.click(
      within(dialog).getByRole("button", { name: "New deck from list" }),
    );
    // Loading the first entry selected its row.
    await ui.user.click(within(dialog).getByLabelText("Selected games (1)"));
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Create deck" }),
    );
    await waitFor(() =>
      expect(ui.api.srsCreateFromRows).toHaveBeenCalledWith(
        "db-1",
        [{ start: "1", end: "2" }],
        "Deck",
        defaultOptions,
        expect.any(Number),
        expect.any(Number),
      ),
    );
  });

  it("shows an error from the backend", async () => {
    const { ui, dialog } = await openDecks([]);
    vi.mocked(ui.api.srsCreateFromPgn).mockRejectedValue({
      message: "The source has no lines to study",
    });
    await ui.user.click(
      within(dialog).getByRole("button", { name: "New deck from PGN…" }),
    );
    await within(dialog).findByLabelText("Name");
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Create deck" }),
    );
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "The source has no lines to study",
    );
  });

  it("edits the options of a deck, with the steps as comma-separated minutes", async () => {
    const { ui, dialog } = await openDecks();
    vi.mocked(ui.api.srsDeck).mockResolvedValue({
      id: "d1",
      name: "Mate",
      source: deck.source,
      sourceKind: "pgn",
      created: 0,
      options: defaultOptions,
      cards: [],
    });
    await ui.user.click(
      await within(dialog).findByRole("button", { name: "Options for Mate" }),
    );
    const options = await screen.findByRole("dialog", { name: "Deck options" });
    const steps = within(options).getByLabelText("Learning steps (minutes)");
    expect(steps).toHaveValue("1, 10");
    await ui.user.clear(steps);
    await ui.user.type(steps, "soon");
    expect(await within(options).findByRole("alert")).toHaveTextContent(
      /learning steps/i,
    );
    expect(
      within(options).getByRole("button", { name: "Save" }),
    ).toBeDisabled();
    await ui.user.clear(steps);
    await ui.user.type(steps, "2, 20");
    const limit = within(options).getByLabelText("New cards per day");
    await ui.user.clear(limit);
    await ui.user.type(limit, "5");
    const name = within(options).getByLabelText("Name");
    await ui.user.clear(name);
    await ui.user.type(name, "Mates");
    await ui.user.click(within(options).getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(ui.api.srsSetOptions).toHaveBeenCalledWith("d1", {
        ...defaultOptions,
        learningSteps: [2, 20],
        newPerDay: 5,
      }),
    );
    expect(ui.api.srsRename).toHaveBeenCalledWith("d1", "Mates");
  });

  it("updates a PGN deck from its file and reports what changed", async () => {
    const { ui, dialog } = await openDecks();
    vi.mocked(ui.api.srsUpdateFromPgn).mockResolvedValue({
      deck,
      skipped: { games: 0, cardsOverLimit: 0, gamesNotRead: 0 },
      added: 3,
      kept: 36,
      suspended: 1,
      restored: 0,
    });
    await ui.user.click(
      await within(dialog).findByRole("button", {
        name: "Update Mate from source",
      }),
    );
    await waitFor(() =>
      expect(ui.api.srsUpdateFromPgn).toHaveBeenCalledWith(
        "d1",
        "/synthetic/Mate.pgn",
        expect.any(Number),
        expect.any(Number),
      ),
    );
    expect(await within(dialog).findByText(/3 added/)).toBeVisible();
    expect(within(dialog).getByText(/1 suspended/)).toBeVisible();
  });

  it("updates a list deck from the games of the open list", async () => {
    const { ui, dialog } = await openDecks([listDeck]);
    vi.mocked(ui.api.srsUpdateFromRows).mockResolvedValue({
      deck: listDeck,
      skipped: { games: 0, cardsOverLimit: 0, gamesNotRead: 0 },
      added: 0,
      kept: 5,
      suspended: 0,
      restored: 0,
    });
    await ui.user.click(
      await within(dialog).findByRole("button", {
        name: "Update From list from source",
      }),
    );
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Update deck" }),
    );
    await waitFor(() =>
      expect(ui.api.srsUpdateFromRows).toHaveBeenCalledWith(
        "d2",
        "db-1",
        [{ start: "1", end: "402" }],
        expect.any(Number),
        expect.any(Number),
      ),
    );
  });

  it("asks before it deletes a deck", async () => {
    const { ui, dialog } = await openDecks();
    const button = await within(dialog).findByRole("button", {
      name: "Delete Mate",
    });
    vi.mocked(ui.api.confirmDeleteDeck).mockResolvedValueOnce(false);
    await ui.user.click(button);
    expect(ui.api.confirmDeleteDeck).toHaveBeenCalledWith("Mate");
    expect(ui.api.srsDelete).not.toHaveBeenCalled();
    await ui.user.click(button);
    await waitFor(() => expect(ui.api.srsDelete).toHaveBeenCalledWith("d1"));
  });

  it("is reachable from the command palette", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(
      await screen.findByRole("button", { name: "Load entry 1" }),
    );
    await ui.user.keyboard("/");
    await ui.user.type(
      await screen.findByRole("textbox", { name: "Command" }),
      "Spaced",
    );
    await ui.user.keyboard("{Enter}");
    expect(
      await screen.findByRole("dialog", { name: "Spaced Repetition" }),
    ).toBeVisible();
  });
});

describe("Study", () => {
  it("plays the opponent's first move by itself and accepts the trainee's move", async () => {
    const { ui, session } = await openStudy();
    expect(within(session).getByText(/Mate\.pgn · game 7/)).toBeVisible();
    expect(within(session).getByText(/Morphy/)).toBeVisible();
    expect(within(session).getByText(/New 0/)).toBeVisible();
    expect(within(session).getByText(/Learning 1/)).toBeVisible();
    expect(within(session).getByText(/Due 2/)).toBeVisible();
    // The board is turned for Black and the opponent has not moved yet.
    expect(within(session).getByRole("grid")).toHaveAttribute(
      "data-flipped",
      "true",
    );
    expect(
      within(session).getByRole("button", { name: "e2 White pawn" }),
    ).toBeVisible();
    await opponentPlayed(session);
    await play(ui, session, "e7", "e5");
    expect(await within(session).findByText("Symmetry")).toBeVisible();
    expect(within(session).getByText("Line complete")).toBeVisible();
    for (const name of ["Again", "Hard", "Good", "Easy"])
      expect(
        within(session).getByRole("button", { name: new RegExp(`^${name}`) }),
      ).toBeEnabled();
  });

  it("tries a new card without showing its moves, and says how many moves mate", async () => {
    const { ui, session } = await openStudy(
      study({ state: newState, mateIn: 1 }),
    );
    expect(
      within(session).getByText("New", { selector: ".srs-badge" }),
    ).toBeVisible();
    expect(within(session).queryByText("Learn")).toBeNull();
    await opponentPlayed(session);
    expect(
      within(session).getByRole("heading", {
        name: "Black to move · Mate in 1",
      }),
    ).toBeVisible();
    expect(within(session).queryByText("e7–e5")).toBeNull();
    await play(ui, session, "e7", "e5");
    expect(await within(session).findByText("Line complete")).toBeVisible();
    for (const name of ["Hard", "Good", "Easy"])
      expect(
        within(session).getByRole("button", { name: new RegExp(`^${name}`) }),
      ).toBeEnabled();
  });

  it("shows the next interval under each grade and sends the grade chosen", async () => {
    const { ui, session } = await openStudy();
    vi.mocked(ui.api.srsAnswer).mockResolvedValue({
      ...queueOf(null),
      state: study().state,
    });
    await opponentPlayed(session);
    await play(ui, session, "e7", "e5");
    const good = await within(session).findByRole("button", { name: /^Good/ });
    expect(good).toHaveTextContent("1d");
    expect(
      within(session).getByRole("button", { name: /^Again/ }),
    ).toHaveTextContent("1m");
    expect(
      within(session).getByRole("button", { name: /^Hard/ }),
    ).toHaveTextContent("6m");
    expect(
      within(session).getByRole("button", { name: /^Easy/ }),
    ).toHaveTextContent("4d");
    await ui.user.click(good);
    await waitFor(() =>
      expect(ui.api.srsAnswer).toHaveBeenCalledWith(...answerOf("good", 0, 0)),
    );
  });

  it("shows the mistake arrow, says so, and forces Again after the line", async () => {
    const { ui, session } = await openStudy();
    vi.mocked(ui.api.srsAnswer).mockResolvedValue({
      ...queueOf(null),
      state: study().state,
    });
    await opponentPlayed(session);
    await play(ui, session, "c7", "c5");
    expect(
      await within(session).findByText("Not this line's move"),
    ).toBeVisible();
    expect(within(session).getByText("e7–e5")).toBeInTheDocument();
    // The wrong move was not played: the pawn is still on c7.
    expect(square(session, "c7")).toHaveAccessibleName("c7 Black pawn");
    await play(ui, session, "e7", "e5");
    expect(await within(session).findByText("Line complete")).toBeVisible();
    expect(
      within(session).getByText(/Again because of a mistake/),
    ).toBeVisible();
    expect(within(session).queryByRole("button", { name: /^Good/ })).toBeNull();
    await ui.user.click(
      within(session).getByRole("button", { name: /Next card/ }),
    );
    await waitFor(() =>
      expect(ui.api.srsAnswer).toHaveBeenCalledWith(...answerOf("again", 1, 0)),
    );
  });

  it("notes a move of another line without a mistake", async () => {
    const { ui, session } = await openStudy();
    vi.mocked(ui.api.srsAnswer).mockResolvedValue({
      ...queueOf(null),
      state: study().state,
    });
    await opponentPlayed(session);
    await play(ui, session, "d7", "d5");
    expect(
      await within(session).findByText(
        "That move belongs to another line in this deck — play this line's move",
      ),
    ).toBeVisible();
    expect(within(session).queryByText("Not this line's move")).toBeNull();
    expect(within(session).queryByText("e7–e5")).toBeNull();
    await play(ui, session, "e7", "e5");
    await ui.user.click(
      await within(session).findByRole("button", { name: /^Good/ }),
    );
    await waitFor(() =>
      expect(ui.api.srsAnswer).toHaveBeenCalledWith(...answerOf("good", 0, 0)),
    );
  });

  it("ignores a move that is not legal", async () => {
    const { ui, session } = await openStudy();
    await opponentPlayed(session);
    await play(ui, session, "e7", "e3");
    expect(within(session).queryByText("Not this line's move")).toBeNull();
    expect(within(session).queryByText("Line complete")).toBeNull();
  });

  it("lets a drag play a move too", async () => {
    const { ui, session } = await openStudy();
    await opponentPlayed(session);
    const board = within(session).getByRole("grid");
    // jsdom has no layout: give the board a square to measure. It is turned
    // for Black, so files run from h to a and ranks from 1 to 8.
    board.getBoundingClientRect = () =>
      ({ left: 0, top: 0, width: 800, height: 800 }) as DOMRect;
    const at = (name: string) => ({
      clientX: "hgfedcba".indexOf(name[0]) * 100 + 50,
      clientY: "12345678".indexOf(name[1]) * 100 + 50,
    });
    fireEvent.pointerDown(square(session, "e7"), {
      button: 0,
      pointerId: 1,
      pointerType: "mouse",
      ...at("e7"),
    });
    fireEvent.pointerMove(board, { pointerId: 1, ...at("e5") });
    fireEvent.pointerUp(board, { pointerId: 1, ...at("e5") });
    expect(await within(session).findByText("Line complete")).toBeVisible();
    expect(ui.api.srsAnswer).not.toHaveBeenCalled();
  });

  it("shows the arrow for a hint and then caps the grade at Hard", async () => {
    const { ui, session } = await openStudy();
    vi.mocked(ui.api.srsAnswer).mockResolvedValue({
      ...queueOf(null),
      state: study().state,
    });
    await opponentPlayed(session);
    expect(within(session).queryByText("e7–e5")).toBeNull();
    await ui.user.click(within(session).getByRole("button", { name: "Hint" }));
    expect(within(session).getByText("e7–e5")).toBeInTheDocument();
    await play(ui, session, "e7", "e5");
    expect(
      await within(session).findByRole("button", { name: /^Good/ }),
    ).toBeDisabled();
    expect(
      within(session).getByRole("button", { name: /^Easy/ }),
    ).toBeDisabled();
    expect(
      within(session).getByRole("button", { name: /^Again/ }),
    ).toBeEnabled();
    await ui.user.click(within(session).getByRole("button", { name: /^Hard/ }));
    await waitFor(() =>
      expect(ui.api.srsAnswer).toHaveBeenCalledWith(...answerOf("hard", 0, 1)),
    );
  });

  it("grades with the keys 1 to 4 and Enter", async () => {
    for (const [key, grade] of [
      ["1", "again"],
      ["2", "hard"],
      ["3", "good"],
      ["4", "easy"],
      ["{Enter}", "good"],
    ]) {
      const { ui, session } = await openStudy();
      vi.mocked(ui.api.srsAnswer).mockResolvedValue({
        ...queueOf(null),
        state: study().state,
      });
      await opponentPlayed(session);
      await play(ui, session, "e7", "e5");
      await within(session).findByText("Line complete");
      await ui.user.keyboard(key);
      await waitFor(() =>
        expect(ui.api.srsAnswer).toHaveBeenCalledWith(...answerOf(grade, 0, 0)),
      );
      ui.unmount();
    }
  });

  it("goes on to the next card from the answer and ends with a summary", async () => {
    const { ui, session } = await openStudy();
    const second = study({
      card: { ...study().card, id: "c2", sourceLabel: "Mate.pgn · game 8" },
    });
    vi.mocked(ui.api.srsAnswer).mockResolvedValue({
      ...queueOf(second),
      state: study().state,
    });
    await opponentPlayed(session);
    await play(ui, session, "e7", "e5");
    await ui.user.click(
      await within(session).findByRole("button", { name: /^Good/ }),
    );
    expect(await within(session).findByText(/game 8/)).toBeVisible();
    await ui.user.click(
      within(session).getByRole("button", { name: "End session" }),
    );
    const summary = within(session).getByText(/Cards studied/);
    expect(summary).toHaveTextContent("Cards studied: 1");
    expect(within(session).getByText(/Again: 0/)).toBeVisible();
    expect(within(session).getByText(/Time: /)).toBeVisible();
    await ui.user.click(within(session).getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog", { name: "Study Mate" })).toBeNull();
  });

  it("ends by itself when nothing more is due", async () => {
    const { ui, session } = await openStudy();
    vi.mocked(ui.api.srsAnswer).mockResolvedValue({
      ...queueOf(null),
      state: study().state,
    });
    await opponentPlayed(session);
    await play(ui, session, "e7", "e5");
    await ui.user.click(
      await within(session).findByRole("button", { name: /^Hard/ }),
    );
    expect(
      await within(session).findByText(/Nothing more is due/),
    ).toBeVisible();
    expect(within(session).getByText(/Cards studied: 1/)).toBeVisible();
  });

  it("counts the Again answers in the summary", async () => {
    const { ui, session } = await openStudy();
    vi.mocked(ui.api.srsAnswer).mockResolvedValue({
      ...queueOf(null),
      state: study().state,
    });
    await opponentPlayed(session);
    await play(ui, session, "e7", "e5");
    await ui.user.click(
      await within(session).findByRole("button", { name: /^Again/ }),
    );
    expect(await within(session).findByText(/Again: 1/)).toBeVisible();
  });

  it("teaches a new card with an arrow for the trainee's move, without mistakes", async () => {
    const { ui, session } = await openStudy(
      study({ learn: true, state: newState }),
    );
    vi.mocked(ui.api.srsAnswer).mockResolvedValue({
      ...queueOf(null),
      state: study().state,
    });
    expect(within(session).getByText("Learn")).toBeVisible();
    await opponentPlayed(session);
    expect(within(session).getByText("e7–e5")).toBeInTheDocument();
    expect(
      within(session).queryByRole("button", { name: "Hint" }),
    ).toBeDisabled();
    await play(ui, session, "c7", "c5");
    expect(within(session).queryByText("Not this line's move")).toBeNull();
    expect(within(session).getByText("e7–e5")).toBeInTheDocument();
    await play(ui, session, "e7", "e5");
    expect(await within(session).findByText("Line complete")).toBeVisible();
    expect(within(session).queryByRole("button", { name: /^Easy/ })).toBeNull();
    await ui.user.click(
      within(session).getByRole("button", { name: /Next card/ }),
    );
    await waitFor(() =>
      expect(ui.api.srsAnswer).toHaveBeenCalledWith(...answerOf("good", 0, 0)),
    );
  });

  it("suspends the card and shows the next one", async () => {
    const { ui, session } = await openStudy();
    const second = study({
      card: { ...study().card, id: "c2", sourceLabel: "Mate.pgn · game 8" },
    });
    vi.mocked(ui.api.srsQueue).mockResolvedValue(queueOf(second));
    await ui.user.click(
      within(session).getByRole("button", { name: "Suspend card" }),
    );
    await waitFor(() =>
      expect(ui.api.srsSuspend).toHaveBeenCalledWith("d1", "c1", true),
    );
    expect(await within(session).findByText(/game 8/)).toBeVisible();
  });

  it("asks which piece to promote to", async () => {
    const fen = "8/4P3/8/8/8/8/8/k1K5 w - - 0 1";
    const { ui, session } = await openStudy(
      study({
        card: {
          ...study().card,
          startFen: fen,
          trainee: "white",
          moves: [{ uci: "e7e8q", san: "e8=Q", comment: null }],
        },
        plies: [
          {
            fen,
            legalUci: ["e7e8q", "e7e8r", "e7e8b", "e7e8n", "c1b1"],
            otherMoves: [],
          },
        ],
        endFen: "4Q3/8/8/8/8/8/8/k1K5 b - - 0 1",
      }),
    );
    await ui.user.click(square(session, "e7"));
    await ui.user.click(square(session, "e8"));
    await ui.user.click(
      within(session).getByRole("button", { name: "Knight" }),
    );
    expect(
      await within(session).findByText("Not this line's move"),
    ).toBeVisible();
    await ui.user.click(square(session, "e7"));
    await ui.user.click(square(session, "e8"));
    await ui.user.click(within(session).getByRole("button", { name: "Queen" }));
    expect(await within(session).findByText("Line complete")).toBeVisible();
  });

  it("reports an error from the backend", async () => {
    const { ui } = await openDecks();
    vi.mocked(ui.api.srsQueue).mockRejectedValue({ message: "Deck is gone" });
    await ui.user.click(
      await screen.findByRole("button", { name: "Study Mate" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("Deck is gone");
  });
});
