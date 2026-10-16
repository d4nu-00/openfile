import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWorkbench } from "./harness";

describe("annotation and medal search", () => {
  it("Search text opens the Annotations tab and searches with medals", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(screen.getByRole("button", { name: "Search text" }));
    const mask = await screen.findByRole("dialog", { name: "Search mask" });
    expect(
      within(mask).getByRole("tab", { name: "Annotations" }),
    ).toHaveAttribute("aria-selected", "true");
    await ui.user.type(within(mask).getByLabelText("Text 1"), "kingside");
    await ui.user.click(within(mask).getByLabelText("Whole word"));
    await ui.user.click(within(mask).getByLabelText("Brilliant move"));
    await ui.user.click(within(mask).getByLabelText("Colours"));
    await ui.user.click(
      within(mask).getByLabelText("Critical endgame position"),
    );
    await ui.user.click(within(mask).getByRole("tab", { name: "Medals" }));
    await ui.user.click(within(mask).getByLabelText("Novelty"));
    await ui.user.click(within(mask).getByLabelText("Endgame"));
    await ui.user.click(within(mask).getByRole("button", { name: "Search" }));
    expect(ui.api.searchGames).toHaveBeenCalledWith(
      "db-1",
      expect.objectContaining({
        // Novelty is bit 3 and Endgame bit 12.
        medals: (1 << 3) | (1 << 12),
        annotations: expect.objectContaining({
          text1: "kingside",
          wholeWord: true,
          symbols: [3],
          colours: true,
          criticalEndgame: true,
          anyText: false,
        }),
      }),
      expect.any(String),
      expect.any(Function),
    );
  });
});
