import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { renderWorkbench } from "./harness";

describe("Deep Analysis", () => {
  it("adds the analysis tree as one edit when it ends", async () => {
    const ui = renderWorkbench();
    await ui.user.click(ui.getByRole("tab", { name: "Analysis" }));
    await ui.user.click(ui.getByRole("button", { name: "Deep Analysis" }));
    const dialog = await ui.findByRole("dialog", { name: "Deep Analysis" });
    const start = within(dialog).getByRole("button", { name: "Start" });
    expect(start).toBeDisabled();
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Other engine…" }),
    );
    await ui.user.click(within(dialog).getByRole("radio", { name: "Minutes" }));
    await ui.user.clear(within(dialog).getByRole("spinbutton"));
    await ui.user.type(within(dialog).getByRole("spinbutton"), "5");
    await ui.user.click(start);
    await waitFor(() =>
      expect(ui.api.deepAnalysis).toHaveBeenCalledWith(
        "doc-1",
        {
          engine: "/synthetic/mock-engine",
          options: {},
          node: 0,
          timeLimitSecs: 300,
        },
        expect.any(Function),
      ),
    );
    await waitFor(() =>
      expect(ui.api.editDocument).toHaveBeenCalledWith(
        "doc-1",
        {
          kind: "addLines",
          parent: 0,
          lines: [
            { moves: ["d2d4", "d7d5"], evaluation: 11, comment: "+0.20/18" },
            { moves: ["c2c4"], evaluation: 14, comment: "+0.35/18" },
          ],
        },
        "0",
      ),
    );
    expect(
      await ui.findByText(
        "Deep Analysis: 1 iteration, 7 positions, 2 lines added",
      ),
    ).toBeVisible();
    expect(ui.queryByRole("dialog", { name: "Deep Analysis" })).toBeNull();
  });

  it("shows progress, and Cancel discards the analysis", async () => {
    const ui = renderWorkbench();
    let finish: (v: unknown) => void = () => {};
    ui.api.deepAnalysis.mockImplementation(
      (_d: string, _p: unknown, onProgress: (p: unknown) => void) => {
        onProgress({
          iteration: 2,
          positions: 30,
          nodes: 40,
          elapsedMs: 75_000,
          best: ["e4", "e5", "Nf3"],
          score: "+0.25",
        });
        return new Promise((resolve) => (finish = resolve));
      },
    );
    await ui.user.click(ui.getByRole("tab", { name: "Analysis" }));
    await ui.user.click(ui.getByRole("button", { name: "Deep Analysis" }));
    const dialog = await ui.findByRole("dialog", { name: "Deep Analysis" });
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Other engine…" }),
    );
    await ui.user.click(within(dialog).getByRole("button", { name: "Start" }));
    expect(
      await within(dialog).findByText(
        "Iteration 2 · 30 positions · 40 moves · 1:15",
      ),
    ).toBeVisible();
    expect(within(dialog).getByText("e4 e5 Nf3 (+0.25)")).toBeVisible();
    await ui.user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(ui.api.stopDeepAnalysis).toHaveBeenCalledWith("doc-1", true);
    finish({
      cancelled: true,
      iterations: 2,
      positions: 30,
      limited: false,
      node: 0,
      lines: [],
      revision: "0",
      best: [],
    });
    expect(
      await within(dialog).findByText("Cancelled; nothing was added."),
    ).toBeVisible();
    expect(ui.api.editDocument).not.toHaveBeenCalled();
  });
});

describe("analysis jobs", () => {
  it("creates jobs from the selected games and runs them", async () => {
    const ui = renderWorkbench({ clean: true });
    ui.api.chooseDatabase.mockResolvedValue("/synthetic/edit.2cbh");
    await ui.user.click(
      screen.getByRole("button", { name: "Open for Editing" }),
    );
    await screen.findByRole("tab", { name: "Database edit" });
    await ui.user.click(
      await screen.findByRole("button", { name: "Load entry 1" }),
    );
    await ui.user.click(
      screen.getByRole("button", { name: "Create analysis jobs" }),
    );
    expect(ui.api.addAnalysisJobs).toHaveBeenCalledWith(
      "db-1",
      [{ start: "1", end: "2" }],
      { type: "analysis", lines: 3, seconds: 30 },
    );
    const dialog = await screen.findByRole("dialog", { name: "Analysis jobs" });
    expect(within(dialog).getByText("White – Black (#1)")).toBeVisible();
    expect(within(dialog).getByText("move 1…")).toBeVisible();
    // ChessBase's minimum time for Deep Analysis jobs is 30 s.
    await ui.user.click(
      within(dialog).getByRole("radio", { name: "Deep Analysis" }),
    );
    await ui.user.clear(
      within(dialog).getByRole("spinbutton", { name: "Time" }),
    );
    await ui.user.type(
      within(dialog).getByRole("spinbutton", { name: "Time" }),
      "20",
    );
    expect(
      within(dialog).getByText(
        "The time must be at least 30 s, as in ChessBase.",
      ),
    ).toBeVisible();
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Other engine…" }),
    );
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Start/Continue" }),
    );
    expect(ui.api.runAnalysisJobs).toHaveBeenCalledWith(
      { engine: "/synthetic/mock-engine", options: {} },
      expect.any(Function),
    );
    expect(
      await within(dialog).findByText(
        "Analysis jobs finished: 1 done, 0 skipped, 0 failed",
      ),
    ).toBeVisible();
  });

  it("adds the board position as a job and edits jobs", async () => {
    const ui = renderWorkbench();
    await ui.user.click(ui.getByRole("tab", { name: "Analysis" }));
    await ui.user.click(ui.getByRole("button", { name: "Add Analysis Job" }));
    expect(ui.api.addAnalysisJob).toHaveBeenCalledWith("doc-1", 0, {
      type: "analysis",
      lines: 3,
      seconds: 30,
    });
    expect(await ui.findByText("Analysis job added")).toBeVisible();
    ui.api.analysisJobs.mockResolvedValue({
      running: false,
      jobs: [
        {
          id: 7,
          databaseId: "db-1",
          ordinal: "4",
          label: "A – B",
          moves: [],
          kind: { type: "deep", seconds: 60 },
          status: "skipped",
          message: "The game is open in a tab",
        },
      ],
    });
    await ui.user.click(ui.getByRole("button", { name: "Open analysis jobs" }));
    const dialog = await ui.findByRole("dialog", { name: "Analysis jobs" });
    expect(
      within(dialog).getByText("Skipped: The game is open in a tab"),
    ).toBeVisible();
    await ui.user.click(within(dialog).getByText("A – B (#4)"));
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Restart" }),
    );
    expect(ui.api.editAnalysisJob).toHaveBeenCalledWith(7, "restart");
    // Nothing waits: Start/Continue is off.
    expect(
      within(dialog).getByRole("button", { name: "Start/Continue" }),
    ).toBeDisabled();
  });
});
