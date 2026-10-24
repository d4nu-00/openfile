import { within, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWorkbench } from "./harness";
import type { RecoveryProgress, RecoveryReport } from "../api/types";

async function dialog() {
  const ui = renderWorkbench({ clean: true, initialView: "database" });
  ui.api.chooseDatabase.mockResolvedValue("/scratch/Damaged.2cbh");
  await ui.user.click(ui.getByRole("tab", { name: "Maintenance" }));
  await ui.user.click(
    ui.getByRole("button", { name: "Recover readable games" }),
  );
  const d = ui.getByRole("dialog", { name: "Recover readable games" });
  await ui.user.click(
    within(d).getByRole("button", { name: "Choose damaged database…" }),
  );
  await ui.user.click(
    within(d).getByRole("button", { name: "Choose new result folder…" }),
  );
  return { ui, d };
}
const recovered: RecoveryReport = {
  checked: 4,
  total: 4,
  recovered: 3,
  corrupt: 1,
  unsupported: 0,
  warnings: 0,
  cancelled: false,
  bytes: 10240,
  problems: [
    { ordinal: 2, code: "corrupt", message: "Block checksum mismatch" },
  ],
  problemsTruncated: false,
  path: "/scratch/Recovered/Recovered.2cbh",
  reportPath: "/scratch/Recovered/recovery.jsonl",
};
describe("recovery from an unopened damaged family", () => {
  it("reports skipped records and opens the verified result only when requested", async () => {
    const { ui, d } = await dialog();
    ui.api.recoverDatabase.mockResolvedValue(recovered);
    expect(d).toHaveTextContent(/source files stay unchanged/i);
    await ui.user.click(within(d).getByRole("button", { name: "Recover" }));
    expect(await within(d).findByText("3 games recovered")).toBeVisible();
    expect(d).toHaveTextContent("Entry 2: Block checksum mismatch");
    expect(ui.api.openDatabase).not.toHaveBeenCalled();
    await ui.user.click(
      within(d).getByRole("button", { name: "Open recovered database" }),
    );
    await waitFor(() =>
      expect(
        ui.queryByRole("dialog", { name: "Recover readable games" }),
      ).toBeNull(),
    );
    expect(ui.api.openDatabase).toHaveBeenCalledWith(
      recovered.path,
      expect.any(String),
    );
  });
  it("ignores stale progress, stops its own job and never offers a cancelled output", async () => {
    const { ui, d } = await dialog();
    let update: ((p: RecoveryProgress) => void) | undefined;
    let finish: ((r: RecoveryReport) => void) | undefined;
    ui.api.recoverDatabase.mockImplementation((_id, _s, _d, onProgress) => {
      update = onProgress;
      return new Promise((resolve) => {
        finish = resolve;
      });
    });
    await ui.user.click(within(d).getByRole("button", { name: "Recover" }));
    await waitFor(() => expect(update).toBeDefined());
    expect(within(d).getByRole("button", { name: "Stop" })).toBeEnabled();
    update!({ jobId: "older", stage: "recovering", done: 999, total: 1000 });
    expect(d).not.toHaveTextContent("999");
    update!({
      jobId: "recovery-test",
      stage: "recovering",
      done: 100,
      total: 1000,
    });
    expect(await within(d).findByText(/100 of 1,000/)).toBeVisible();
    await ui.user.click(within(d).getByRole("button", { name: "Stop" }));
    expect(ui.api.cancelRecovery).toHaveBeenCalledWith("recovery-test");
    finish!({ ...recovered, cancelled: true, path: null, reportPath: null });
    expect(
      await within(d).findByText(/Stopped; no result was published/),
    ).toBeVisible();
    expect(
      within(d).queryByRole("button", { name: "Open recovered database" }),
    ).toBeNull();
  });
  it("retains choices after a destination failure and permits retry", async () => {
    const { ui, d } = await dialog();
    ui.api.recoverDatabase.mockRejectedValueOnce(
      new Error("The result folder already exists"),
    );
    await ui.user.click(within(d).getByRole("button", { name: "Recover" }));
    expect(await within(d).findByRole("alert")).toHaveTextContent(
      "already exists",
    );
    expect(d).toHaveTextContent("/scratch/Recovered");
    expect(within(d).getByRole("button", { name: "Recover" })).toBeEnabled();
  });
});
