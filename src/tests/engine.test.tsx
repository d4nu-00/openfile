import { it, expect } from "vitest";
import { act, waitFor, within } from "@testing-library/react";
import { renderWorkbench } from "./harness";
import { formatScore } from "../state/analysis";
import type { EngineEventDto } from "../api/types";
it("shows root scores from White’s perspective and inverts bounds", () => {
  expect(formatScore({ kind: "cp", value: -31 }, "black", "white")).toBe(
    "+0.31",
  );
  expect(formatScore({ kind: "mate", moves: 3 }, "black", "white")).toBe("-M3");
  expect(
    formatScore(
      { kind: "cp", value: -31, bound: "lowerbound" },
      "black",
      "white",
    ),
  ).toBe("≤+0.31");
  expect(
    formatScore(
      { kind: "mate", moves: -2, bound: "upperbound" },
      "white",
      "white",
    ),
  ).toBe("≤-M2");
});
it("keeps engine selection cancellation and executable errors separate from the game", async () => {
  const ui = renderWorkbench();
  ui.api.pickEngine.mockResolvedValue(null);
  await ui.user.click(ui.getByRole("button", { name: "Add kibitzer" }));
  expect(ui.api.chooseEngine).not.toHaveBeenCalled();
  ui.api.pickEngine.mockResolvedValue("/missing/engine");
  ui.api.chooseEngine.mockRejectedValue({ message: "Executable missing" });
  await ui.user.click(ui.getByRole("button", { name: "Add kibitzer" }));
  expect(await ui.findByText("Executable missing")).toBeVisible();
  expect(ui.getByRole("heading", { name: "Alpha — Beta" })).toBeVisible();
});
it("validates engine options before sending and supports spaced combo values", async () => {
  const ui = renderWorkbench();
  await ui.user.click(ui.getByRole("button", { name: "Add kibitzer" }));
  await ui.user.click(await ui.findByText("Engine options"));
  const threads = await ui.findByRole("spinbutton", { name: "Threads" });
  await ui.user.clear(threads);
  await ui.user.type(threads, "0");
  await ui.user.click(ui.getByRole("button", { name: "Apply Threads" }));
  expect(ui.api.setEngineOption).not.toHaveBeenCalled();
  expect(ui.getByText(/between 1 and 64/)).toBeVisible();
  await ui.user.clear(threads);
  await ui.user.type(threads, "4");
  await ui.user.click(ui.getByRole("button", { name: "Apply Threads" }));
  expect(ui.api.setEngineOption).toHaveBeenCalledWith(
    "engine-1",
    "Threads",
    "4",
  );
  await ui.user.selectOptions(
    ui.getByRole("combobox", { name: "Playing Style" }),
    "Very Solid",
  );
  await ui.user.click(ui.getByRole("button", { name: "Apply Playing Style" }));
  expect(ui.api.setEngineOption).toHaveBeenLastCalledWith(
    "engine-1",
    "Playing Style",
    "Very Solid",
  );
});
// A kibitzer starts analysing as soon as it is added.
async function start(ui: ReturnType<typeof renderWorkbench>) {
  await ui.user.click(ui.getByRole("button", { name: "Add kibitzer" }));
  await waitFor(() => expect(ui.api.startAnalysis).toHaveBeenCalled());
}
function emit(
  ui: ReturnType<typeof renderWorkbench>,
  call: number,
  nodes: string,
  score = -31,
): EngineEventDto {
  const args = ui.api.startAnalysis.mock.calls[call];
  const event: EngineEventDto = {
    engineId: args[0],
    requestId: args[3],
    payload: {
      kind: "info",
      depth: 18,
      nodes,
      multipv: 1,
      score: { kind: "cp", value: score },
      rootTurn: call ? "black" : "white",
      pv: ["e5"],
      rawPv: ["e7e5"],
      diagnostics: [],
    },
  };
  act(() => args[4](event));
  return event;
}
it("starts fresh requests on node changes and suppresses old channel events", async () => {
  const ui = renderWorkbench();
  await start(ui);
  emit(ui, 0, "111");
  await ui.user.click(ui.getByRole("button", { name: "e4" }));
  await waitFor(() => expect(ui.api.startAnalysis).toHaveBeenCalledTimes(2));
  emit(ui, 0, "999", 999);
  emit(ui, 1, "9007199254740993");
  expect(await ui.findByText("+0.31")).toBeVisible();
  expect(ui.getByText("9007199254740993")).toBeVisible();
  expect(ui.queryByText("999")).toBeNull();
  expect(ui.getByText("Score from White's perspective")).toBeVisible();
  await ui.user.click(ui.getByRole("button", { name: "Stop analysis" }));
  expect(ui.api.stopAnalysis).toHaveBeenCalledWith("engine-1");
  emit(ui, 1, "888");
  await new Promise((r) => setTimeout(r, 150));
  expect(ui.queryByText("888")).toBeNull();
});
it("shows mate and invalid PV diagnostics and recovers after process failure", async () => {
  const ui = renderWorkbench();
  await start(ui);
  const args = ui.api.startAnalysis.mock.calls[0];
  act(() =>
    args[4]({
      engineId: "engine-1",
      requestId: args[3],
      payload: {
        kind: "info",
        multipv: 2,
        rootTurn: "white",
        score: { kind: "mate", moves: 3 },
        nodes: "123",
        depth: 22,
        pv: [],
        rawPv: ["a1a8"],
        diagnostics: ["Illegal PV move a1a8"],
      },
    }),
  );
  expect(await ui.findByText("+M3")).toBeVisible();
  expect(ui.getByText("Illegal PV move a1a8")).toBeVisible();
  act(() =>
    args[4]({
      engineId: "engine-1",
      requestId: args[3],
      payload: { kind: "error", message: "Engine process exited" },
    }),
  );
  expect(await ui.findByText("Engine process exited")).toBeVisible();
  expect(ui.getByRole("heading", { name: "Alpha — Beta" })).toBeVisible();
  ui.api.chooseEngine.mockResolvedValue({
    engineId: "engine-2",
    identity: { name: "Recovered" },
    options: [],
    status: "ready",
  });
  await ui.user.click(ui.getByRole("button", { name: "Add kibitzer" }));
  await waitFor(() =>
    expect(ui.api.startAnalysis).toHaveBeenLastCalledWith(
      "engine-2",
      "doc-1",
      0,
      expect.any(String),
      expect.any(Function),
    ),
  );
  ui.unmount();
  expect(ui.api.closeEngine).toHaveBeenCalledWith("engine-2");
});
it("retains the latest line for each MultiPV across visual flushes", async () => {
  const ui = renderWorkbench();
  await start(ui);
  emit(ui, 0, "111");
  expect(await ui.findByText("111")).toBeVisible();
  const args = ui.api.startAnalysis.mock.calls[0];
  act(() =>
    args[4]({
      engineId: "engine-1",
      requestId: args[3],
      payload: {
        kind: "info",
        multipv: 2,
        rootTurn: "white",
        nodes: "222",
        depth: 15,
        pv: ["d4"],
        rawPv: ["d2d4"],
        diagnostics: [],
      },
    }),
  );
  expect(await ui.findByText("222")).toBeVisible();
  expect(ui.getByText("111")).toBeVisible();
});
it("closes the engine and clears its subscription while retaining the game", async () => {
  const ui = renderWorkbench();
  await start(ui);
  await ui.user.click(ui.getByRole("button", { name: "Close engine" }));
  expect(ui.api.closeEngine).toHaveBeenCalledWith("engine-1");
  expect(ui.queryByRole("button", { name: "Start analysis" })).toBeNull();
  expect(ui.getByRole("heading", { name: "Alpha — Beta" })).toBeVisible();
  emit(ui, 0, "closed-event");
  await new Promise((r) => setTimeout(r, 150));
  expect(ui.queryByText("closed-event")).toBeNull();
});
it.each(["stop", "failure"])(
  "clears retained analysis when the board changes after %s",
  async (mode) => {
    const ui = renderWorkbench();
    await start(ui);
    emit(ui, 0, "111");
    expect(await ui.findByText("-0.31")).toBeVisible();
    if (mode === "stop")
      await ui.user.click(ui.getByRole("button", { name: "Stop analysis" }));
    else {
      const args = ui.api.startAnalysis.mock.calls[0];
      act(() =>
        args[4]({
          engineId: args[0],
          requestId: args[3],
          payload: { kind: "error", message: "Engine process exited" },
        }),
      );
      expect(await ui.findByText("Engine process exited")).toBeVisible();
    }
    expect(ui.getByText("-0.31")).toBeVisible();
    await ui.user.click(ui.getByRole("button", { name: "e4" }));
    await waitFor(() =>
      expect(ui.api.positionAt).toHaveBeenLastCalledWith(
        "doc-1",
        1,
        expect.any(String),
      ),
    );
    await waitFor(() => expect(ui.queryByText("-0.31")).toBeNull());
    expect(ui.queryByText("111")).toBeNull();
  },
);
it("merges partial engine fields and clears an explicitly supplied invalid PV", async () => {
  const ui = renderWorkbench();
  await start(ui);
  emit(ui, 0, "111");
  expect(await ui.findByText("-0.31")).toBeVisible();
  const args = ui.api.startAnalysis.mock.calls[0];
  act(() =>
    args[4]({
      engineId: args[0],
      requestId: args[3],
      payload: {
        kind: "info",
        multipv: 1,
        rootTurn: "white",
        nodes: "222",
        diagnostics: [],
      },
    }),
  );
  expect(await ui.findByText("222")).toBeVisible();
  expect(ui.getByText("-0.31")).toBeVisible();
  const table = within(ui.getByRole("table", { name: "Engine analysis" }));
  expect(table.getByText("18")).toBeVisible();
  expect(table.getByText("e5")).toBeVisible();
  act(() =>
    args[4]({
      engineId: args[0],
      requestId: args[3],
      payload: {
        kind: "info",
        multipv: 1,
        rootTurn: "white",
        pv: [],
        rawPv: ["a1a8"],
        diagnostics: ["Invalid supplied PV"],
      },
    }),
  );
  expect(await ui.findByText("Invalid supplied PV")).toBeVisible();
  expect(table.queryByText("e5")).toBeNull();
  expect(ui.getByText("-0.31")).toBeVisible();
});

/** Engines with MultiPV, each with its own id. */
function multipvEngines(ui: ReturnType<typeof renderWorkbench>) {
  let n = 0;
  ui.api.chooseEngine.mockImplementation(async () => ({
    engineId: `engine-${++n}`,
    identity: { name: `Engine ${n}` },
    options: [
      {
        name: "MultiPV",
        kind: "spin",
        default: "1",
        min: 1,
        max: 500,
        vars: [],
      },
    ],
    status: "ready",
  }));
}
it("changes the number of lines and restarts the analysis", async () => {
  const ui = renderWorkbench();
  multipvEngines(ui);
  await start(ui);
  await ui.user.keyboard("+");
  await waitFor(() =>
    expect(ui.api.setEngineOption).toHaveBeenCalledWith(
      "engine-1",
      "MultiPV",
      "2",
    ),
  );
  expect(ui.api.stopAnalysis).toHaveBeenCalledWith("engine-1");
  await waitFor(() => expect(ui.api.startAnalysis).toHaveBeenCalledTimes(2));
  expect(ui.getByLabelText("Lines")).toHaveTextContent("2");
  await ui.user.keyboard("-");
  await waitFor(() =>
    expect(ui.api.setEngineOption).toHaveBeenLastCalledWith(
      "engine-1",
      "MultiPV",
      "1",
    ),
  );
});
it("pastes the engine's line and move into the notation", async () => {
  const ui = renderWorkbench();
  await start(ui);
  const args = ui.api.startAnalysis.mock.calls[0];
  act(() =>
    args[4]({
      engineId: args[0],
      requestId: args[3],
      payload: {
        kind: "info",
        multipv: 1,
        depth: 22,
        rootTurn: "white",
        score: { kind: "cp", value: 35 },
        pv: ["d4", "d5"],
        rawPv: ["d2d4", "d7d5"],
        diagnostics: [],
      },
    }),
  );
  await ui.findByText("d4 d5");
  await ui.user.keyboard("{Control>} {/Control}");
  await waitFor(() =>
    expect(ui.api.editDocument).toHaveBeenCalledWith(
      "doc-1",
      {
        kind: "appendLine",
        parent: 0,
        moves: ["d2d4", "d7d5"],
        comment: "+0.35/22",
      },
      "0",
    ),
  );
  await ui.user.keyboard("{Shift>} {/Shift}");
  await waitFor(() =>
    expect(ui.api.editDocument).toHaveBeenLastCalledWith(
      "doc-1",
      { kind: "appendLine", parent: 0, moves: ["d2d4"] },
      expect.any(String),
    ),
  );
});
it("runs several kibitzers and removes them one by one or all", async () => {
  const ui = renderWorkbench();
  multipvEngines(ui);
  await start(ui);
  await ui.user.click(ui.getByRole("button", { name: "Add kibitzer" }));
  await waitFor(() =>
    expect(ui.api.startAnalysis).toHaveBeenLastCalledWith(
      "engine-2",
      "doc-1",
      0,
      expect.any(String),
      expect.any(Function),
    ),
  );
  expect(ui.getByRole("region", { name: "Kibitzer Engine 1" })).toBeVisible();
  expect(ui.getByRole("region", { name: "Kibitzer Engine 2" })).toBeVisible();
  // Remove Kibitzer takes the last one used (the last added).
  await ui.user.keyboard("{Control>}{Shift>}k{/Shift}{/Control}");
  expect(ui.api.closeEngine).toHaveBeenCalledWith("engine-2");
  expect(ui.queryByRole("region", { name: "Kibitzer Engine 2" })).toBeNull();
  await ui.user.click(ui.getByRole("button", { name: "Add kibitzer" }));
  await ui.findByRole("region", { name: "Kibitzer Engine 3" });
  await ui.user.click(ui.getByRole("tab", { name: "Home" }));
  const ribbon = ui.getByRole("button", { name: "Remove All Kibitzers" });
  await ui.user.click(ribbon);
  await waitFor(() =>
    expect(ui.queryByRole("region", { name: /Kibitzer/ })).toBeNull(),
  );
  expect(ui.api.closeEngine).toHaveBeenCalledWith("engine-1");
  expect(ui.api.closeEngine).toHaveBeenCalledWith("engine-3");
});
it("installs engines, keeps a default and toggles the default kibitzer", async () => {
  const ui = renderWorkbench();
  multipvEngines(ui);
  await ui.user.click(ui.getByRole("tab", { name: "Home" }));
  await ui.user.click(ui.getByRole("button", { name: "Engine Management" }));
  const dialog = await ui.findByRole("dialog", { name: "Engine Management" });
  await ui.user.click(
    within(dialog).getByRole("button", { name: "Add engine…" }),
  );
  expect(ui.api.probeEngine).toHaveBeenCalledWith("/synthetic/mock-engine");
  // The first engine installed becomes the default.
  expect(
    await within(dialog).findByRole("radio", {
      name: "Default engine Mock engine 1.0",
    }),
  ).toBeChecked();
  expect(ui.api.setSettings).toHaveBeenCalledWith(
    expect.objectContaining({
      engines: [
        expect.objectContaining({
          name: "Mock engine 1.0",
          path: "/synthetic/mock-engine",
        }),
      ],
    }),
  );
  await ui.user.click(within(dialog).getByRole("button", { name: "Close" }));
  // Alt+F2 starts the default engine, and stops it again.
  await ui.user.keyboard("{Alt>}{F2}{/Alt}");
  await waitFor(() =>
    expect(ui.api.chooseEngine).toHaveBeenCalledWith("/synthetic/mock-engine"),
  );
  await ui.findByRole("region", { name: "Kibitzer Engine 1" });
  await ui.user.keyboard("{Alt>}{F2}{/Alt}");
  await waitFor(() =>
    expect(ui.api.closeEngine).toHaveBeenCalledWith("engine-1"),
  );
});
it("runs Tactical Analysis and adds its findings as one edit", async () => {
  const ui = renderWorkbench();
  await ui.user.keyboard("{Control>}{Alt>}{Shift>}u{/Shift}{/Alt}{/Control}");
  const dialog = await ui.findByRole("dialog", { name: "Tactical Analysis" });
  const start = within(dialog).getByRole("button", { name: "Start" });
  // No engine installed yet: one is chosen as a file.
  expect(start).toBeDisabled();
  await ui.user.click(
    within(dialog).getByRole("button", { name: "Other engine…" }),
  );
  await ui.user.click(within(dialog).getByRole("radio", { name: "White" }));
  await ui.user.click(
    within(dialog).getByRole("checkbox", { name: "Store evaluations" }),
  );
  await ui.user.click(start);
  await waitFor(() =>
    expect(ui.api.blunderCheck).toHaveBeenCalledWith(
      "doc-1",
      {
        engine: "/synthetic/mock-engine",
        options: {},
        // Time 3 [s] × 500 ms; Fine is 0.35.
        millisPerMove: 1500,
        threshold: 35,
        side: "white",
      },
      expect.any(Function),
    ),
  );
  // A loss of 1.80 pawns is over three times the threshold: "??".
  await waitFor(() =>
    expect(ui.api.editDocument).toHaveBeenCalledWith(
      "doc-1",
      {
        kind: "addAnalysis",
        items: [
          {
            node: 1,
            nag: 4,
            evaluation: 17,
            comment: "-1.50",
            line: ["d2d4", "d7d5"],
            lineComment: "+0.30/20",
          },
          // An evaluation symbol alone, where the evaluation changed.
          { node: 2, evaluation: 11, line: [] },
        ],
      },
      "0",
    ),
  );
  expect(
    await ui.findByText("Tactical Analysis: 1 mistakes in 2 positions"),
  ).toBeVisible();
  expect(ui.queryByRole("dialog", { name: "Tactical Analysis" })).toBeNull();
});
it("adds nothing when Tactical Analysis is stopped", async () => {
  const ui = renderWorkbench();
  ui.api.blunderCheck.mockResolvedValue({
    cancelled: true,
    checked: 1,
    findings: [],
    evaluations: [],
    revision: "0",
  });
  await ui.user.keyboard("{Control>}{Alt>}{Shift>}u{/Shift}{/Alt}{/Control}");
  const dialog = await ui.findByRole("dialog", { name: "Tactical Analysis" });
  await ui.user.click(
    within(dialog).getByRole("button", { name: "Other engine…" }),
  );
  await ui.user.click(within(dialog).getByRole("button", { name: "Start" }));
  expect(
    await within(dialog).findByText("Stopped; nothing was added."),
  ).toBeVisible();
  expect(ui.api.editDocument).not.toHaveBeenCalled();
});

it("annotates the game from the tablebases as one edit", async () => {
  const ui = renderWorkbench();
  await ui.user.click(ui.getByRole("tab", { name: "Analysis" }));
  await ui.user.click(ui.getByRole("button", { name: "Tablebase Annotation" }));
  expect(ui.api.tablebaseAnnotation).toHaveBeenCalledWith("doc-1");
  await waitFor(() =>
    expect(ui.api.editDocument).toHaveBeenCalledWith(
      "doc-1",
      {
        kind: "addAnalysis",
        items: [
          {
            node: 1,
            nag: 2,
            comment: "Tablebase: draw",
            line: ["d2d4"],
            lineComment: "Tablebase: White wins",
          },
        ],
      },
      "0",
    ),
  );
  expect(
    await ui.findByText(
      "Tablebase Annotation: 2 positions in the tables, 1 move gives away the result",
    ),
  ).toBeVisible();
});
