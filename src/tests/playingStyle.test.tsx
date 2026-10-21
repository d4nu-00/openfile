import { act, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { StyleReport } from "../api/types";
import { renderWorkbench } from "./harness";

export const styleReport: StyleReport = {
  jobId: "test",
  players: [{ id: 3, name: "Player 3, A" }],
  criteria: { playerIds: [3], sampleLimit: 500 },
  database: "Synthetic",
  databaseEntries: "401",
  eligible: 12,
  selected: 12,
  analysed: 10,
  whiteGames: 6,
  blackGames: 4,
  deleted: 1,
  filtered: 2,
  skipped: 2,
  cutGames: 0,
  firstDate: "2020.01.01",
  lastDate: "2024.12.31",
  elapsedMs: 123,
  metrics: [
    {
      id: "checks",
      label: "Checking moves",
      description: "Own moves giving check / all own moves.",
      numerator: 0,
      denominator: 240,
      value: 0,
      minimum: 100,
      white: { numerator: 0, denominator: 140, value: 0 },
      black: { numerator: 0, denominator: 100, value: 0 },
      evidence: [],
    },
    {
      id: "captures",
      label: "Capturing moves",
      description: "Own captures / all own moves.",
      numerator: 40,
      denominator: 240,
      value: 100 / 6,
      minimum: 100,
      white: { numerator: 24, denominator: 140, value: 120 / 7 },
      black: { numerator: 16, denominator: 100, value: 16 },
      evidence: [{ ordinal: "3", node: 1, ply: 1, san: "e4" }],
    },
    {
      id: "endgame",
      label: "Endgames reached",
      description: "Both sides have at most 13 nonpawn points.",
      numerator: 0,
      denominator: 0,
      value: null,
      minimum: 5,
      white: { numerator: 0, denominator: 0, value: null },
      black: { numerator: 0, denominator: 0, value: null },
      evidence: [],
    },
  ],
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
  it("sends explicit criteria, shows measured zeros and missing evidence, and opens a supporting position", async () => {
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
        sampleLimit: 80,
        yearFrom: 2020,
        white: false,
      }),
      expect.any(Function),
    );
    expect(
      await within(ui.dialog).findByText(/10 games analysed/),
    ).toBeVisible();
    const checks = within(ui.dialog).getByRole("region", {
      name: "Checking moves",
    });
    expect(
      within(checks).getByText("0.0%", { selector: "strong" }),
    ).toBeVisible();
    expect(within(checks).getByText("0 / 240")).toBeVisible();
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Structure & endings" }),
    );
    expect(
      within(ui.dialog).getAllByText("Insufficient evidence").length,
    ).toBeGreaterThan(0);
    await ui.user.click(
      within(ui.dialog).getByRole("button", { name: "Activity & openings" }),
    );
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
