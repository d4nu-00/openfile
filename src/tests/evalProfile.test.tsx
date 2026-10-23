import { it, expect } from "vitest";
import { act, waitFor, within } from "@testing-library/react";
import { renderWorkbench } from "./harness";
import { criticalPlies } from "../state/evalProfile";
import { plyName } from "../components/EvalProfile";
import type { InstantProgress } from "../api/types";

it("marks moves that lose a pawn for the side that played them", () => {
  const points = [
    { ply: 0, cp: 20, depth: 1 },
    { ply: 1, cp: 30, depth: 1 },
    // Black's move: White gains 1.20, so Black lost it.
    { ply: 2, cp: 150, depth: 1 },
    // White's move: down to +0.60, a loss of 0.90 (not critical).
    { ply: 3, cp: 60, depth: 1 },
    // White's move lets Black mate.
    { ply: 4, cp: 0, depth: 1 },
    { ply: 5, mate: -2, depth: 1 },
    // Black's move from −9 to −7.5 still leaves Black far ahead.
    { ply: 7, cp: -900, depth: 1 },
    { ply: 8, cp: -750, depth: 1 },
  ];
  expect(criticalPlies(points, true)).toEqual([2, 5]);
  expect(plyName(1, "e4", true)).toBe("1.e4");
  expect(plyName(2, "e5", true)).toBe("1...e5");
  expect(plyName(1, "e5", false)).toBe("1...e5");
});

it("runs instant analysis on the shown game and jumps to its critical move", async () => {
  const ui = renderWorkbench({
    settings: {
      engines: [{ id: "sf", name: "Engine", path: "/e", options: {} }],
      defaultEngine: "sf",
    },
  });
  // Instant Analysis also shows the profile pane.
  await ui.user.click(ui.getByRole("tab", { name: "Analysis" }));
  await ui.user.click(
    await ui.findByRole("button", { name: /Instant Analysis/ }),
  );
  await waitFor(() => expect(ui.api.instantAnalysis).toHaveBeenCalled(), {
    timeout: 2000,
  });
  const [doc, params, onProgress] = ui.api.instantAnalysis.mock.calls[0];
  expect(doc).toBe("doc-1");
  expect(params).toEqual({
    engine: "/e",
    options: {},
    level: 3,
    endless: false,
  });
  const progress: InstantProgress = {
    revision: "0",
    pass: 1,
    millisPerPosition: 105,
    done: 3,
    total: 3,
    profile: [
      { ply: 0, cp: 20, depth: 1 },
      { ply: 1, cp: 30, depth: 1 },
      { ply: 2, cp: 180, depth: 1 },
    ],
    finished: false,
    cancelled: false,
  };
  act(() => onProgress(progress));
  const pane = await ui.findByLabelText("Evaluation profile");
  expect(
    within(pane).getByText(/pass 1, 3\/3 positions at 105 ms/),
  ).toBeVisible();
  expect(within(pane).getByText("1 critical move")).toBeVisible();
  await ui.user.click(
    within(pane).getByRole("button", { name: "Critical move 1...e5" }),
  );
  await waitFor(() =>
    expect(ui.api.positionAt.mock.calls.at(-1)?.slice(0, 2)).toEqual([
      "doc-1",
      2,
    ]),
  );
  // Stop cancels it in the backend.
  await ui.user.click(within(pane).getByRole("button", { name: "Stop" }));
  expect(ui.api.cancelInstantAnalysis).toHaveBeenCalled();
});

it("shows the profile stored with the game in the notation", async () => {
  const ui = renderWorkbench();
  await ui.user.click(ui.getByRole("tab", { name: "View" }));
  await ui.user.click(
    await ui.findByRole("button", { name: /Show Evaluation Profile/ }),
  );
  expect(
    await ui.findByText("No evaluations for this game yet."),
  ).toBeVisible();
  expect(ui.api.instantAnalysis).not.toHaveBeenCalled();
});
