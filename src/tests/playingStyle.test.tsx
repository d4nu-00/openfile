import { act, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { StyleReport } from "../api/types";
import { renderWorkbench } from "./harness";

export const styleReport: StyleReport = {
  jobId: "test",
  players: [{ id: 3, name: "Player 3, A" }],
  criteria: { playerIds: [3], sampleLimit: 500, gameTypes: 4 },
  database: "Synthetic",
  databaseEntries: "401",
  eligible: 12,
  selected: 12,
  analysed: 10,
  whiteGames: 6,
  blackGames: 4,
  deleted: 1,
  filtered: 2,
  tooShort: 1,
  unknownCategory: 0,
  skipped: 2,
  cutGames: 0,
  firstDate: "2020.01.01",
  lastDate: "2024.12.31",
  elapsedMs: 123,
  dimensions: [
    ["score", "Score", 0, 60],
    ["theory", "Theory", null, null],
    ["decidedGames", "Decided Games", 2.5, 200],
    ["tenacity", "Tenacity", null, null],
    ["aggressiveness", "Aggressiveness", null, null],
    ["risk", "Risk", null, null],
    ["positionalPlay", "Positional Play", null, null],
    ["endgameAffinity", "Endgame Affinity", null, null],
  ].map(([id, label, rawScore, normalized]) => ({
    id: id as string,
    label: label as string,
    description: "Recovered native composite calculation.",
    rawScore: rawScore as number | null,
    normalized: normalized as number | null,
    normalization: (id === "decidedGames" ? [-0.5, 1] : [-1.5, 1]) as [
      number,
      number,
    ],
    grade: null,
    gradeReason: "Native categorical grading is not verified.",
    status: rawScore == null ? "unavailable" : "available",
    unavailableReason:
      rawScore == null
        ? "Requires native positional inputs that are not yet reproducible."
        : null,
    inputs:
      rawScore == null
        ? []
        : [
            {
              label: "White points per game",
              numerator: 5.5,
              denominator: 10,
              value: 0.55,
              reference: 0.55,
            },
          ],
    evidence:
      id === "score" ? [{ ordinal: "3", node: 1, ply: 1, san: "e4" }] : [],
  })),
  games: [
    {
      ordinal: "3",
      white: true,
      opponent: "Bravo, B",
      date: "2024.12.31",
      result: "*",
      plies: 24,
    },
  ],
};

async function openStyle() {
  const ui = renderWorkbench({ initialView: "database" });
  await ui.user.click(screen.getByRole("tab", { name: "Players" }));
  await ui.user.click(
    await screen.findByRole("button", { name: "Choose Player 3, A" }),
  );
  await ui.user.click(screen.getByRole("button", { name: "Style report" }));
  return {
    ...ui,
    dialog: await screen.findByRole("dialog", { name: "Style report" }),
  };
}

describe("local playing style", () => {
  it("shows the eight native dimensions, separates recovered values from unavailable inputs, and opens supporting evidence", async () => {
    const ui = await openStyle();
    ui.api.playingStyle.mockImplementation(async (_db, jobId, params) => ({
      ...styleReport,
      jobId,
      criteria: params,
    }));
    await ui.user.clear(within(ui.dialog).getByLabelText("Maximum games"));
    await ui.user.type(within(ui.dialog).getByLabelText("Maximum games"), "80");
    await ui.user.type(within(ui.dialog).getByLabelText("From year"), "2020");
    await ui.user.selectOptions(
      within(ui.dialog).getByLabelText("Colour"),
      "black",
    );
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Generate report" }),
    );
    expect(ui.api.playingStyle).toHaveBeenCalledWith(
      "db-1",
      expect.any(String),
      expect.objectContaining({
        playerIds: [3],
        gameTypes: 4,
        sampleLimit: 80,
        yearFrom: 2020,
        white: false,
      }),
      expect.any(Function),
    );
    expect(
      await within(ui.dialog).findByText(/10 games analysed/),
    ).toBeVisible();
    const score = within(ui.dialog).getByRole("region", { name: "Score" });
    expect(
      within(score).getByText("60.0%", { selector: "strong" }),
    ).toBeVisible();
    expect(within(score).getByText(/Raw score.*0\.000/)).toBeVisible();
    expect(
      within(score).getByText("Categorical grade unavailable"),
    ).toBeVisible();
    for (const label of [
      "Theory",
      "Decided Games",
      "Tenacity",
      "Aggressiveness",
      "Risk",
      "Positional Play",
      "Endgame Affinity",
    ])
      expect(
        within(ui.dialog).getByRole("region", { name: label }),
      ).toBeInTheDocument();
    expect(
      within(
        within(ui.dialog).getByRole("region", { name: "Decided Games" }),
      ).getByText("200.0%", { selector: "strong" }),
    ).toBeVisible();
    expect(within(ui.dialog).getByText(/Small sample:/)).toBeVisible();
    const theory = within(ui.dialog).getByRole("region", { name: "Theory" });
    expect(within(theory).getByText("—", { selector: "strong" })).toBeVisible();
    expect(
      within(theory).getByText(/Requires native positional inputs/),
    ).toBeVisible();
    expect(
      within(ui.dialog).queryByRole("region", { name: "Checking moves" }),
    ).toBeNull();
    await ui.user.click(within(score).getByText("Calculation & inputs"));
    expect(within(score).getByText("White points per game")).toBeVisible();
    await ui.user.click(
      within(ui.dialog).getByText("Supporting positions (1 shown)"),
    );
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: /Entry 3.*e4/ }),
    );
    expect(ui.api.loadEntry).toHaveBeenCalledWith(
      "db-1",
      "3",
      expect.any(String),
    );
    expect(ui.api.loadStyleEntry).toHaveBeenCalledWith(
      "db-1",
      expect.any(String),
      "3",
      expect.any(String),
    );
    expect(
      await screen.findByRole("grid", { name: "Chess board" }),
    ).toBeVisible();
    await waitFor(() =>
      expect(ui.api.positionAt).toHaveBeenCalledWith(
        expect.any(String),
        1,
        expect.any(String),
      ),
    );
  });

  it("defaults to Long games and requires at least one native game category", async () => {
    const ui = await openStyle();
    ui.api.playingStyle.mockImplementation(async (_db, jobId, criteria) => ({
      ...styleReport,
      jobId,
      criteria,
    }));
    const generate = within(ui.dialog).getByRole("button", {
      name: "Generate report",
    });
    expect(
      within(ui.dialog).getByRole("checkbox", { name: "Long" }),
    ).toBeChecked();
    await ui.user.click(
      within(ui.dialog).getByRole("checkbox", { name: "Long" }),
    );
    expect(generate).toBeDisabled();
    await ui.user.click(
      within(ui.dialog).getByRole("checkbox", { name: "Blitz / bullet" }),
    );
    expect(generate).toBeEnabled();
    await ui.user.click(generate);
    expect(ui.api.playingStyle).toHaveBeenCalledWith(
      "db-1",
      expect.any(String),
      expect.objectContaining({ gameTypes: 1 }),
      expect.any(Function),
    );
    expect(
      await within(ui.dialog).findByText(
        /2 of 8 dimensions have a computed score/,
      ),
    ).toBeVisible();
  });

  it("rejects a late result and progress after Stop and a newer generation", async () => {
    const ui = await openStyle();
    let finish!: (r: StyleReport) => void;
    let oldProgress!: (p: {
      stage: string;
      done: string;
      total: string;
    }) => void;
    ui.api.playingStyle
      .mockImplementationOnce((_db, _jobId, _params, p) => {
        oldProgress = p;
        return new Promise<StyleReport>((resolve) => {
          finish = resolve;
        });
      })
      .mockImplementationOnce(async (_db, jobId) => ({
        ...styleReport,
        jobId,
        analysed: 99,
      }));
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Generate report" }),
    );
    const firstJob = ui.api.playingStyle.mock.calls[0][1];
    // WebKit can apply the click default after React changes this button
    // back to type=submit. Stopping must suppress that default submission.
    const stopClick = new MouseEvent("click", {
      bubbles: true,
      cancelable: true,
    });
    await act(async () => {
      within(ui.dialog)
        .getByRole("button", { name: "Stop" })
        .dispatchEvent(stopClick);
    });
    expect(stopClick.defaultPrevented).toBe(true);
    expect(ui.api.playingStyle).toHaveBeenCalledTimes(1);
    expect(ui.api.cancelPlayingStyle).toHaveBeenCalledWith("db-1", firstJob);
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Generate report" }),
    );
    expect(
      await within(ui.dialog).findByText(/99 games analysed/),
    ).toBeVisible();
    await act(async () => {
      oldProgress({ stage: "scan", done: "1", total: "100" });
      finish({ ...styleReport, jobId: firstJob });
    });
    expect(within(ui.dialog).getByText(/99 games analysed/)).toBeVisible();
    expect(
      within(ui.dialog).queryByRole("button", { name: "Stop" }),
    ).toBeNull();
    expect(ui.api.playingStyle.mock.calls[1][1]).not.toBe(firstJob);
  });

  it("closes safely while running and leaves the report absent after completion", async () => {
    const ui = await openStyle();
    let finish!: (r: StyleReport) => void;
    ui.api.playingStyle.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Generate report" }),
    );
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Close" }),
    );
    await act(async () => finish(styleReport));
    expect(screen.queryByRole("dialog", { name: "Style report" })).toBeNull();
    expect(ui.api.cancelPlayingStyle).toHaveBeenCalled();
  });
  it("lets the user explicitly add an alias, without automatically merging similar names", async () => {
    const ui = await openStyle();
    ui.api.readNames.mockResolvedValueOnce({
      requestId: "alias",
      rows: [
        {
          id: 7,
          name: "Player 3, Alternative",
          games: 2,
          firstYear: 2020,
          lastYear: 2021,
        },
      ],
    });
    await ui.user.click(
      within(ui.dialog).getByText("Include another spelling / identity"),
    );
    await ui.user.type(
      within(ui.dialog).getByLabelText("Alias name"),
      "Player 3",
    );
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Find spelling" }),
    );
    const alias = await within(ui.dialog).findByRole("button", {
      name: /Player 3, Alternative.*#7/,
    });
    expect(ui.api.playingStyle).not.toHaveBeenCalled();
    await ui.user.click(alias);
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Generate report" }),
    );
    expect(ui.api.playingStyle).toHaveBeenCalledWith(
      "db-1",
      expect.any(String),
      expect.objectContaining({ playerIds: [3, 7] }),
      expect.any(Function),
    );
  });

  it("refuses evidence navigation when the source has changed", async () => {
    const ui = await openStyle();
    ui.api.playingStyle.mockImplementationOnce(async (_db, jobId) => ({
      ...styleReport,
      jobId,
    }));
    ui.api.validatePlayingStyle.mockRejectedValueOnce({
      message: "The database changed. Generate a new report.",
    });
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Generate report" }),
    );
    await within(ui.dialog).findByText(/10 games analysed/);
    await ui.user.click(
      within(ui.dialog).getByText("Supporting positions (1 shown)"),
    );
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: /Entry 3.*e4/ }),
    );
    expect(await within(ui.dialog).findByRole("alert")).toHaveTextContent(
      "The database changed",
    );
    expect(ui.api.loadEntry).not.toHaveBeenCalled();
  });

  it("lists all analysed entries separately from the current search and reports backend errors", async () => {
    const ui = await openStyle();
    ui.api.playingStyle
      .mockRejectedValueOnce({ message: "Invalid year range" })
      .mockImplementationOnce(async (_db, jobId) => ({
        ...styleReport,
        jobId,
      }));
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Generate report" }),
    );
    expect(await within(ui.dialog).findByRole("alert")).toHaveTextContent(
      "Invalid year range",
    );
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Generate report" }),
    );
    await within(ui.dialog).findByText(/10 games analysed/);
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Analysed games" }),
    );
    expect(within(ui.dialog).getByText("Bravo, B")).toBeVisible();
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Open 3" }),
    );
    await waitFor(() =>
      expect(ui.api.loadEntry).toHaveBeenCalledWith(
        "db-1",
        "3",
        expect.any(String),
      ),
    );
    expect(ui.api.validatePlayingStyle).toHaveBeenCalledWith(
      "db-1",
      expect.any(String),
    );
  });
  it("ignores evidence validation that finishes after a newer report starts", async () => {
    const ui = await openStyle();
    ui.api.playingStyle
      .mockImplementationOnce(async (_db, jobId) => ({ ...styleReport, jobId }))
      .mockImplementationOnce(() => new Promise(() => {}));
    let complete!: () => void;
    ui.api.validatePlayingStyle.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          complete = resolve;
        }),
    );
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Generate report" }),
    );
    await within(ui.dialog).findByText(/10 games analysed/);
    await ui.user.click(
      within(ui.dialog).getByText("Supporting positions (1 shown)"),
    );
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: /Entry 3.*e4/ }),
    );
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Generate again" }),
    );
    await act(async () => complete());
    expect(screen.getByRole("dialog", { name: "Style report" })).toBeVisible();
    expect(
      within(ui.dialog).getByRole("button", { name: "Stop" }),
    ).toBeVisible();
    expect(ui.api.loadEntry).not.toHaveBeenCalled();
  });
});
