import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { it, expect } from "vitest";
import { App } from "../App";
import { mockApi } from "./harness";
it("mounts and reaches the selected game commands", async () => {
  const api = mockApi();
  render(<App api={api} />);
  await userEvent.click(screen.getByRole("button", { name: "Open database" }));
  await userEvent.click(
    await screen.findByRole("button", { name: "Load entry 1" }),
  );
  expect(await screen.findByText("Alpha — Beta")).toBeVisible();
  expect(api.positionAt).toHaveBeenCalledWith("doc-1", 0, expect.any(String));
});
it("renders large decimal entry counts without rounding", async () => {
  const api = mockApi();
  api.openDatabase.mockImplementationOnce(async (_path, requestId) => ({
    requestId,
    databaseId: "db-1",
    entryCount: "9007199254740993",
    companions: [],
    diagnostics: [],
  }));
  render(<App api={api} />);
  await userEvent.click(screen.getByRole("button", { name: "Open database" }));
  expect((await screen.findAllByText("9007199254740993 entries")).length).toBe(
    2,
  );
});
it("displays a backend open error", async () => {
  const api = mockApi();
  api.openDatabase.mockRejectedValueOnce({
    code: "io",
    message: "Source unavailable",
  });
  render(<App api={api} />);
  await userEvent.click(screen.getByRole("button", { name: "Open database" }));
  expect(await screen.findByText("Source unavailable")).toBeVisible();
});
