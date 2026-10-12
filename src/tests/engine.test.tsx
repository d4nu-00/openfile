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
  await ui.user.click(ui.getByRole("button", { name: "Choose engine" }));
  expect(ui.api.chooseEngine).not.toHaveBeenCalled();
  ui.api.pickEngine.mockResolvedValue("/missing/engine");
  ui.api.chooseEngine.mockRejectedValue({ message: "Executable missing" });
  await ui.user.click(ui.getByRole("button", { name: "Choose engine" }));
  expect(await ui.findByText("Executable missing")).toBeVisible();
  expect(ui.getByRole("heading", { name: "Alpha — Beta" })).toBeVisible();
});
it("validates engine options before sending and supports spaced combo values", async () => {
  const ui = renderWorkbench();
  await ui.user.click(ui.getByRole("button", { name: "Choose engine" }));
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
async function start(ui: ReturnType<typeof renderWorkbench>) {
  await ui.user.click(ui.getByRole("button", { name: "Choose engine" }));
  await ui.user.click(
    await ui.findByRole("button", { name: "Start analysis" }),
  );
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
  await ui.user.click(ui.getByRole("button", { name: "Choose engine" }));
  await ui.user.click(
    await ui.findByRole("button", { name: "Start analysis" }),
  );
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
