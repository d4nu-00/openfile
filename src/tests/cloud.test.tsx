import { describe, expect, it } from "vitest";
import { renderWorkbench } from "./harness";

describe("databases in iCloud", () => {
  it("asks, downloads with progress, then opens", async () => {
    const ui = renderWorkbench({ clean: true });
    ui.api.cloudStatus.mockResolvedValueOnce({
      files: ["test database.cbg", "test database.cba"],
      bytes: "1500000000",
    });
    await ui.user.keyboard("{Control>}o{/Control}");
    await expect.poll(() => ui.api.openDatabase.mock.calls.length).toBe(1);
    expect(ui.api.confirmCloudDownload).toHaveBeenCalledWith(
      expect.any(String),
      2,
      "1.5 GB",
    );
    expect(ui.api.downloadCloudFiles).toHaveBeenCalled();
    expect(ui.api.openDatabase).toHaveBeenCalled();
  });

  it("does not open when the download is declined", async () => {
    const ui = renderWorkbench({ clean: true });
    ui.api.cloudStatus.mockResolvedValueOnce({
      files: ["test database.cbg"],
      bytes: "1253435766",
    });
    ui.api.confirmCloudDownload.mockResolvedValueOnce(false);
    await ui.user.keyboard("{Control>}o{/Control}");
    await expect
      .poll(() => ui.api.confirmCloudDownload.mock.calls.length)
      .toBe(1);
    expect(ui.api.downloadCloudFiles).not.toHaveBeenCalled();
    expect(ui.api.openDatabase).not.toHaveBeenCalled();
  });

  it("reports a stopped download as not opened", async () => {
    const ui = renderWorkbench({ clean: true });
    ui.api.cloudStatus.mockResolvedValueOnce({
      files: ["test database.cbg"],
      bytes: "1253435766",
    });
    ui.api.downloadCloudFiles.mockRejectedValueOnce({
      code: "cancelled",
      message: "Stopped waiting for iCloud; the database was not opened",
    });
    await ui.user.keyboard("{Control>}o{/Control}");
    expect(await ui.findByText(/Stopped waiting for iCloud/)).toBeVisible();
    expect(ui.api.openDatabase).not.toHaveBeenCalled();
  });
});
