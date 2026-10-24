import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { AppSettings } from "../api/types";
import { normaliseCombination } from "../state/mateTraining";
import { renderWorkbench } from "./harness";

const engine = {
  id: "sf",
  name: "Stockfish",
  path: "/e/stockfish",
  options: {},
};

async function open(
  settings: Partial<AppSettings> = { engines: [engine], defaultEngine: "sf" },
) {
  const ui = renderWorkbench({ initialView: "database", settings });
  await ui.user.click(
    await screen.findByRole("button", { name: "Load entry 1" }),
  );
  await ui.user.click(
    screen.getByRole("button", { name: "Build Mate Training" }),
  );
  const dialog = await screen.findByRole("dialog", {
    name: "Build Mate Training",
  });
  return { ui, dialog };
}

describe("combinations as typed", () => {
  it("are put in piece order whatever the case and spacing", () => {
    expect(normaliseCombination("n + q", false)).toBe("Q+N");
    expect(normaliseCombination(" r+k ", false)).toBe("R+K");
    expect(normaliseCombination("k+p+b", false)).toBe("B+P+K");
    expect(normaliseCombination("q+q", false)).toBe("Q");
  });
  it("write the minor pieces as m when bishops and knights are pooled", () => {
    expect(normaliseCombination("n+q", true)).toBe("Q+m");
    expect(normaliseCombination("b+n", true)).toBe("m");
    expect(normaliseCombination("Q+M", true)).toBe("Q+m");
    expect(normaliseCombination("Q+M", false)).toBeNull();
  });
  it("refuse an unknown letter or an empty part", () => {
    for (const bad of ["", "+", "x+q", "q+", "qr", "q++r", "q r"])
      expect(normaliseCombination(bad, false), bad).toBeNull();
  });
});

describe("Build Mate Training", () => {
  it("opens from the database ribbon with its defaults", async () => {
    const { dialog } = await open();
    expect(within(dialog).getByLabelText("From depth")).toHaveValue("1");
    expect(within(dialog).getByLabelText("To depth")).toHaveValue("5");
    expect(
      within(dialog).getByLabelText("Puzzles per combination"),
    ).toHaveValue(20);
    expect(within(dialog).getByLabelText("Time per check (ms)")).toHaveValue(
      1000,
    );
    expect(within(dialog).getByLabelText("Name")).toHaveValue("Mate training");
    expect(within(dialog).getByLabelText("Engine")).toHaveValue("sf");
    expect(
      within(dialog).getByRole("radio", { name: /All games in the list/ }),
    ).toBeChecked();
    expect(
      within(dialog).getByRole("checkbox", { name: "Bishop = Knight" }),
    ).not.toBeChecked();
    // Build waits for a folder.
    expect(
      within(dialog).getByRole("button", { name: "Build" }),
    ).toBeDisabled();
  });

  it("sends the ranges, options, engine, time, folder and name", async () => {
    const { ui, dialog } = await open();
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Choose…" }),
    );
    expect(await within(dialog).findByText("/synthetic/out")).toBeVisible();
    await ui.user.selectOptions(within(dialog).getByLabelText("To depth"), "3");
    await ui.user.clear(
      within(dialog).getByLabelText("Puzzles per combination"),
    );
    await ui.user.type(
      within(dialog).getByLabelText("Puzzles per combination"),
      "7",
    );
    await ui.user.click(
      within(dialog).getByRole("radio", { name: "Only these" }),
    );
    await ui.user.type(
      within(dialog).getByLabelText("Combination list"),
      "n + q, R+K",
    );
    await ui.user.click(
      within(dialog).getByRole("checkbox", { name: "Bishop = Knight" }),
    );
    await ui.user.clear(within(dialog).getByLabelText("Time per check (ms)"));
    await ui.user.type(
      within(dialog).getByLabelText("Time per check (ms)"),
      "800",
    );
    await ui.user.clear(within(dialog).getByLabelText("Name"));
    await ui.user.type(within(dialog).getByLabelText("Name"), "Mega mates");
    await ui.user.click(within(dialog).getByRole("button", { name: "Build" }));
    expect(ui.api.buildMateTraining).toHaveBeenCalledWith(
      "db-1",
      [{ start: "1", end: expect.stringMatching(/^\d+$/) }],
      {
        minDepth: 1,
        maxDepth: 3,
        per: 7,
        combinations: ["Q+m", "R+K"],
        poolMinors: true,
      },
      "sf",
      800,
      "/synthetic/out",
      "Mega mates",
      expect.any(Function),
    );
    const [, ranges] = ui.api.buildMateTraining.mock.calls[0];
    // The whole list: its rows are counted from 1, the end is exclusive.
    expect(Number(ranges[0].end) - 1).toBeGreaterThan(3);
  });

  it("sends no list of combinations unless asked", async () => {
    const { ui, dialog } = await open();
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Choose…" }),
    );
    await within(dialog).findByText("/synthetic/out");
    await ui.user.click(within(dialog).getByRole("button", { name: "Build" }));
    expect(ui.api.buildMateTraining.mock.calls[0][2]).toEqual({
      minDepth: 1,
      maxDepth: 5,
      per: 20,
      combinations: null,
      poolMinors: false,
    });
  });

  it("uses the selected rows when asked", async () => {
    const ui = renderWorkbench({
      initialView: "database",
      settings: { engines: [engine], defaultEngine: "sf" },
    });
    await ui.user.click(
      await screen.findByRole("button", { name: "Load entry 1" }),
    );
    await ui.user.keyboard("{Shift>}");
    await ui.user.click(screen.getByRole("button", { name: "Load entry 3" }));
    await ui.user.keyboard("{/Shift}");
    await ui.user.click(
      screen.getByRole("button", { name: "Build Mate Training" }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Build Mate Training",
    });
    expect(
      within(dialog).getByRole("radio", { name: "Selected games (3)" }),
    ).toBeChecked();
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Choose…" }),
    );
    await within(dialog).findByText("/synthetic/out");
    await ui.user.click(within(dialog).getByRole("button", { name: "Build" }));
    expect(ui.api.buildMateTraining.mock.calls[0][1]).toEqual([
      { start: "1", end: "4" },
    ]);
  });

  it("disables Build for a combination with an unknown piece", async () => {
    const { ui, dialog } = await open();
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Choose…" }),
    );
    await within(dialog).findByText("/synthetic/out");
    await ui.user.click(
      within(dialog).getByRole("radio", { name: "Only these" }),
    );
    const build = within(dialog).getByRole("button", { name: "Build" });
    // An empty list asks for nothing.
    expect(build).toBeDisabled();
    await ui.user.type(
      within(dialog).getByLabelText("Combination list"),
      "Q+N, X+R",
    );
    expect(build).toBeDisabled();
    expect(within(dialog).getByRole("alert")).toHaveTextContent(
      '"X+R" is not a combination such as Q+N',
    );
    await ui.user.clear(within(dialog).getByLabelText("Combination list"));
    await ui.user.type(
      within(dialog).getByLabelText("Combination list"),
      "Q+N",
    );
    expect(build).toBeEnabled();
    expect(within(dialog).queryByRole("alert")).toBeNull();
  });

  it("needs an engine for depths above two", async () => {
    const { ui, dialog } = await open({
      engines: [],
      defaultEngine: undefined,
    });
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Choose…" }),
    );
    await within(dialog).findByText("/synthetic/out");
    expect(within(dialog).getByText(/Add Kibitzer/)).toBeVisible();
    expect(
      within(dialog).getByRole("button", { name: "Build" }),
    ).toBeDisabled();
    // Mates in 1 and 2 are solved exactly.
    await ui.user.selectOptions(within(dialog).getByLabelText("To depth"), "2");
    await ui.user.click(within(dialog).getByRole("button", { name: "Build" }));
    expect(ui.api.buildMateTraining.mock.calls[0][3]).toBe("");
  });

  it("shows the progress, and Stop and Cancel call the backend", async () => {
    const { ui, dialog } = await open();
    let finish: (r: unknown) => void = () => {};
    ui.api.buildMateTraining.mockImplementation(
      async (_d, _r, _o, _e, _m, _f, _n, onProgress) => {
        onProgress({
          scanned: "200",
          total: "400",
          puzzles: [3, 2, 1, 0, 0],
          chains: 2,
          combinations: [],
        });
        return new Promise((resolve) => (finish = resolve));
      },
    );
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Choose…" }),
    );
    await within(dialog).findByText("/synthetic/out");
    await ui.user.click(within(dialog).getByRole("button", { name: "Build" }));
    expect(await within(dialog).findByRole("status")).toHaveTextContent(
      "200 of 400 games",
    );
    expect(within(dialog).getByRole("status")).toHaveTextContent(
      "Mate in 1: 3",
    );
    expect(within(dialog).getByRole("status")).toHaveTextContent("2 chains");
    await ui.user.click(within(dialog).getByRole("button", { name: "Stop" }));
    expect(ui.api.stopMateTraining).toHaveBeenCalledWith("db-1");
    await ui.user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(ui.api.cancelMateTraining).toHaveBeenCalledWith("db-1");
    finish({
      databases: [],
      combinations: [],
      puzzles: [0, 0, 0, 0, 0],
      chains: 0,
      rejections: [],
      scanned: "200",
      total: "400",
      filteredOut: "0",
      stopped: false,
      cancelled: true,
      complete: false,
    });
    expect(await within(dialog).findByText(/Cancelled/)).toBeVisible();
  });

  it("summarises the counts and opens a database it wrote", async () => {
    const { ui, dialog } = await open();
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Choose…" }),
    );
    await within(dialog).findByText("/synthetic/out");
    await ui.user.click(within(dialog).getByRole("button", { name: "Build" }));
    const table = await within(dialog).findByRole("table", {
      name: "Puzzles by combination",
    });
    const row = (name: string) =>
      within(table).getByRole("row", { name: new RegExp(`^${name}`) });
    expect(
      within(row("Q\\+N"))
        .getAllByRole("cell")
        .map((c) => c.textContent),
    ).toEqual(["3", "0", "0", "0", "0", "1"]);
    expect(within(row("R\\+K")).getAllByRole("cell")[0]).toHaveTextContent("2");
    expect(
      within(table).getByRole("row", { name: /^Total/ }),
    ).toHaveTextContent("5");
    expect(within(dialog).getByText(/1 chain/)).toBeVisible();
    expect(within(dialog).getByText("not a forced mate: 12")).toBeVisible();
    expect(
      within(dialog).getByText("a mate with another move: 4"),
    ).toBeVisible();
    expect(
      within(dialog).getByText("/synthetic/out/Mate training - Mate in 1.2cbh"),
    ).toBeVisible();
    await ui.user.click(
      within(dialog).getByRole("button", {
        name: "Open Mate training - Mate chains.2cbh",
      }),
    );
    await waitFor(() =>
      expect(ui.api.openDatabase).toHaveBeenLastCalledWith(
        "/synthetic/out/Mate training - Mate chains.2cbh",
        expect.any(String),
      ),
    );
  });
});
