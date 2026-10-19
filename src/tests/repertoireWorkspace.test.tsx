import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RepertoirePane } from "../components/RepertoirePane";
import type { WorkbenchApi } from "../api/client";
import type { RepertoireSnapshot } from "../api/types";
import { renderWorkbench } from "./harness";

const line = {
  startFen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
  moves: [],
};
const snapshot: RepertoireSnapshot = {
  entries: [],
  current: undefined,
  ownTurn: true,
  legal: [
    { uci: "e2e4", san: "e4", preferred: false, note: "", valid: true },
    { uci: "d2d4", san: "d4", preferred: false, note: "", valid: true },
  ],
};

describe("Repertoire workspace", () => {
  it("saves a legal move and labels uncovered reference counts", async () => {
    const edit = vi.fn().mockResolvedValue({
      ...snapshot,
      current: {
        key: "start",
        fen: line.startFen,
        line,
        route: "Start position",
        ownTurn: true,
        practice: { attempts: 0, correct: 0, needsRetry: false },
        moves: [
          { uci: "e2e4", san: "e4", preferred: true, note: "", valid: true },
        ],
      },
    });
    const api = {
      repertoireSnapshot: vi.fn().mockResolvedValue(snapshot),
      repertoireEdit: edit,
    } as unknown as WorkbenchApi;
    render(
      <RepertoirePane
        api={api}
        line={line}
        side="white"
        book={{
          loading: false,
          tree: {
            requestId: "r",
            status: "tree",
            games: 12,
            ended: 0,
            moves: [
              { uci: "e2e4", san: "e4", games: 8, share: 67 },
              { uci: "d2d4", san: "d4", games: 4, share: 33 },
            ],
          },
        }}
        onNavigate={() => {}}
        onPractice={() => {}}
      />,
    );
    await screen.findByRole("button", { name: "Save e4" });
    fireEvent.click(screen.getByRole("button", { name: "Save e4" }));
    await waitFor(() =>
      expect(edit).toHaveBeenCalledWith(
        line,
        "white",
        "e2e4",
        "add",
        undefined,
      ),
    );
    expect(screen.getByText(/d4.*4 games/)).toBeInTheDocument();
  });

  it("hides saved choices in practice and offers another attempt after a miss", async () => {
    const current = {
      key: "start",
      fen: line.startFen,
      line,
      route: "Start position",
      ownTurn: true,
      practice: { attempts: 1, correct: 0, needsRetry: true },
      moves: [
        {
          uci: "e2e4",
          san: "e4",
          preferred: true,
          note: "Centre",
          valid: true,
        },
      ],
    };
    const api = {
      repertoireSnapshot: vi
        .fn()
        .mockResolvedValue({ ...snapshot, current, entries: [current] }),
    } as unknown as WorkbenchApi;
    const onPractice = vi.fn();
    render(
      <RepertoirePane
        api={api}
        line={line}
        side="white"
        book={{ loading: false }}
        practice={{
          correct: false,
          playedSan: "d4",
          preferredSan: "e4",
          practice: current.practice,
        }}
        onNavigate={() => {}}
        onPractice={onPractice}
      />,
    );
    await screen.findByRole("button", { name: "Try again" });
    expect(screen.queryByText("Centre")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(onPractice).toHaveBeenCalled();
  });

  it("keeps editing available without a tree and marks invalid saved moves", async () => {
    const current = {
      key: "start",
      fen: line.startFen,
      line,
      route: "Start position",
      ownTurn: false,
      practice: { attempts: 0, correct: 0, needsRetry: false },
      moves: [
        {
          uci: "e2e5",
          san: "e2e5",
          preferred: true,
          note: "Old",
          valid: false,
        },
      ],
    };
    const api = {
      repertoireSnapshot: vi.fn().mockResolvedValue({
        ...snapshot,
        ownTurn: false,
        current,
        entries: [current],
      }),
      repertoireEdit: vi.fn().mockResolvedValue(snapshot),
    } as unknown as WorkbenchApi;
    render(
      <RepertoirePane
        api={api}
        line={line}
        side="black"
        book={{
          loading: false,
          tree: {
            requestId: "r",
            status: "noTree",
            games: 0,
            ended: 0,
            moves: [],
          },
        }}
        onNavigate={() => {}}
        onPractice={() => {}}
      />,
    );
    expect(await screen.findByText("Invalid saved move")).toBeVisible();
    expect(screen.getByText(/Reference data missing/)).toBeVisible();
    expect(
      screen.getByRole("heading", { name: "Opponent replies in my lines" }),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Remove e2e5" }));
    await waitFor(() =>
      expect(api.repertoireEdit).toHaveBeenCalledWith(
        line,
        "black",
        "e2e5",
        "remove",
        undefined,
      ),
    );
  });

  it("labels bounded reference counts as sampled", async () => {
    const api = {
      repertoireSnapshot: vi.fn().mockResolvedValue(snapshot),
    } as unknown as WorkbenchApi;
    render(
      <RepertoirePane
        api={api}
        line={line}
        side="white"
        book={{
          loading: false,
          tree: {
            requestId: "r",
            status: "filtered",
            games: 50,
            ended: 0,
            moves: [],
            sampledHits: 50,
            totalHits: 100,
          },
        }}
        onNavigate={() => {}}
        onPractice={() => {}}
      />,
    );
    expect(
      await screen.findByText(/Sampled search result: 50 of 100/),
    ).toBeVisible();
  });
});

it("grades a board move in practice without annotating the open game", async () => {
  const ui = renderWorkbench();
  vi.spyOn(ui.api, "repertoireSnapshot").mockResolvedValue({
    ...snapshot,
    current: {
      key: "start",
      fen: line.startFen,
      line,
      route: "Start position",
      ownTurn: true,
      moves: [
        { uci: "e2e4", san: "e4", preferred: true, note: "", valid: true },
      ],
      practice: { attempts: 0, correct: 0, needsRetry: false },
    },
  });
  const grade = vi.spyOn(ui.api, "repertoireGrade").mockResolvedValue({
    correct: true,
    playedSan: "e4",
    preferredSan: "e4",
    practice: { attempts: 1, correct: 1, needsRetry: false },
  });
  await ui.user.click(ui.getByRole("tab", { name: "Repertoire" }));
  await ui.user.click(
    await ui.findByRole("button", { name: "Practise this position" }),
  );
  expect(ui.queryByText("Preferred")).not.toBeInTheDocument();
  await ui.user.click(ui.getByRole("button", { name: "e2 White pawn" }));
  await ui.user.click(ui.getByRole("button", { name: "e4 empty" }));
  await waitFor(() =>
    expect(grade).toHaveBeenCalledWith(line, "white", "e2e4"),
  );
  expect(ui.api.editDocument).not.toHaveBeenCalled();
  expect(await ui.findByText("Correct: e4.")).toBeVisible();
});

it("uses the database tree for coverage even while a player preparation is open", async () => {
  const ui = renderWorkbench({ initialView: "database" });
  vi.spyOn(ui.api, "repertoireSnapshot").mockResolvedValue(snapshot);
  await ui.user.click(
    await screen.findByRole("button", { name: "Load entry 3" }),
  );
  await waitFor(() => expect(ui.api.loadEntry).toHaveBeenCalled());
  await ui.user.click(screen.getByRole("tab", { name: "Players" }));
  await ui.user.click(
    await screen.findByRole("button", { name: "Choose Player 3, A" }),
  );
  await ui.user.keyboard("{Control>}b{/Control}");
  const dialog = await screen.findByRole("dialog", {
    name: "Prepare against Black",
  });
  await ui.user.click(
    await within(dialog).findByRole("button", { name: "Show in Book" }),
  );
  await waitFor(() => expect(ui.api.preparedTree).toHaveBeenCalled());
  await ui.user.click(screen.getByRole("tab", { name: "Repertoire" }));
  await waitFor(() => expect(ui.api.openingTree).toHaveBeenCalled());
});

it("plays a normal game move after leaving practice", async () => {
  const ui = renderWorkbench();
  const grade = vi.spyOn(ui.api, "repertoireGrade");
  vi.spyOn(ui.api, "repertoireSnapshot").mockResolvedValue({
    ...snapshot,
    current: {
      key: "start",
      fen: line.startFen,
      line,
      route: "Start position",
      ownTurn: true,
      moves: [
        { uci: "e2e4", san: "e4", preferred: true, note: "", valid: true },
      ],
      practice: { attempts: 0, correct: 0, needsRetry: false },
    },
  });
  await ui.user.click(ui.getByRole("tab", { name: "Repertoire" }));
  await ui.user.click(
    await ui.findByRole("button", { name: "Practise this position" }),
  );
  await ui.user.click(ui.getByRole("tab", { name: "Notation" }));
  await ui.user.click(ui.getByRole("button", { name: "e2 White pawn" }));
  await ui.user.click(ui.getByRole("button", { name: "e4 empty" }));
  await waitFor(() => expect(ui.api.editDocument).toHaveBeenCalled());
  expect(grade).not.toHaveBeenCalled();
});

it("counts an opponent reply as uncovered until my next move is saved", async () => {
  const api = {
    repertoireSnapshot: vi.fn().mockResolvedValue({
      ...snapshot,
      ownTurn: false,
      answeredReplies: { e7e5: true, c7c5: false },
    }),
  } as unknown as WorkbenchApi;
  render(
    <RepertoirePane
      api={api}
      line={{ ...line, moves: ["e2e4"] }}
      side="white"
      book={{
        loading: false,
        tree: {
          requestId: "r",
          status: "tree",
          games: 10,
          ended: 0,
          moves: [
            { uci: "e7e5", san: "e5", games: 6, share: 60 },
            { uci: "c7c5", san: "c5", games: 4, share: 40 },
          ],
        },
      }}
      onNavigate={() => {}}
      onPractice={() => {}}
    />,
  );
  expect(await screen.findByText(/c5.*4 games/)).toBeVisible();
  expect(screen.queryByText(/e5.*6 games/)).not.toBeInTheDocument();
});

it("does not show an old edit result after switching repertoire colour", async () => {
  let resolveEdit!: (value: RepertoireSnapshot) => void;
  const edit = vi.fn().mockImplementation(
    () =>
      new Promise<RepertoireSnapshot>((resolve) => {
        resolveEdit = resolve;
      }),
  );
  const api = {
    repertoireSnapshot: vi.fn().mockResolvedValue(snapshot),
    repertoireEdit: edit,
  } as unknown as WorkbenchApi;
  const props = {
    api,
    line,
    book: { loading: false },
    onNavigate: () => {},
    onPractice: () => {},
  };
  const ui = render(<RepertoirePane {...props} side="white" />);
  await screen.findByRole("button", { name: "Save e4" });
  fireEvent.click(screen.getByRole("button", { name: "Save e4" }));
  await waitFor(() => expect(edit).toHaveBeenCalled());
  ui.rerender(<RepertoirePane {...props} side="black" />);
  await waitFor(() =>
    expect(api.repertoireSnapshot).toHaveBeenCalledWith(line, "black"),
  );
  await act(async () =>
    resolveEdit({
      ...snapshot,
      current: {
        key: "start",
        fen: line.startFen,
        line,
        route: "Start position",
        ownTurn: true,
        moves: [
          { uci: "e2e4", san: "e4", preferred: true, note: "", valid: true },
        ],
        practice: { attempts: 0, correct: 0, needsRetry: false },
      },
    }),
  );
  expect(
    screen.queryByRole("button", { name: "Remove e4" }),
  ).not.toBeInTheDocument();
});
