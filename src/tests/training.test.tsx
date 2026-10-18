import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWorkbench } from "./harness";

describe("training annotations", () => {
  it("adds a question on a move, opens it from the notation and deletes it", async () => {
    const ui = renderWorkbench();
    await ui.user.click(await ui.findByRole("button", { name: "e5" }));
    await ui.user.keyboard("{Control>}{Alt>}m{/Alt}{/Control}");
    const dialog = await ui.findByRole("dialog", {
      name: "Training Annotation",
    });
    expect(
      within(dialog).getByText("Training Annotation: before 1...e5"),
    ).toBeVisible();
    const save = within(dialog).getByRole("button", { name: "Save" });
    expect(save).toBeDisabled();
    await ui.user.type(
      within(dialog).getByLabelText("Question"),
      "How does Black answer?",
    );
    // The move played is the first answer.
    expect(within(dialog).getByLabelText("Move 1")).toHaveValue("e5");
    await ui.user.type(within(dialog).getByLabelText("Feedback 1"), "Right");
    await ui.user.click(
      within(dialog).getByRole("button", { name: "Add answer" }),
    );
    await ui.user.type(within(dialog).getByLabelText("Move 2"), "c5");
    await ui.user.click(save);
    await waitFor(() =>
      expect(ui.api.editDocument).toHaveBeenCalledWith(
        "doc-1",
        expect.objectContaining({
          kind: "setTraining",
          node: 2,
          moves: ["e5", "c5"],
          training: expect.objectContaining({
            kind: "moveInput",
            seconds: 300,
            score: 10,
            question: {
              texts: [
                { language: 0, index: 0, text: "How does Black answer?" },
              ],
            },
          }),
        }),
        expect.any(String),
      ),
    );
    expect(
      screen.queryByRole("dialog", { name: "Training Annotation" }),
    ).toBeNull();

    // The notation shows the question; its chip opens it again.
    await ui.user.click(await ui.findByRole("button", { name: "Training" }));
    const again = await ui.findByRole("dialog", {
      name: "Training Annotation",
    });
    expect(within(again).getByLabelText("Question")).toHaveValue(
      "How does Black answer?",
    );
    await ui.user.click(within(again).getByRole("button", { name: "Delete" }));
    await waitFor(() =>
      expect(ui.api.editDocument).toHaveBeenLastCalledWith(
        "doc-1",
        { kind: "setTraining", node: 2, training: null, moves: [] },
        expect.any(String),
      ),
    );
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Training" })).toBeNull(),
    );
  });
});
