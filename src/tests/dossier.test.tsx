import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWorkbench } from "./harness";

describe("player dossier", () => {
  it("builds the chosen player's dossier and opens its rows", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(screen.getByRole("tab", { name: "Players" }));
    await ui.user.click(
      await screen.findByRole("button", { name: "Choose Player 3, A" }),
    );
    await ui.user.keyboard("{Control>}{Alt>}d{/Alt}{/Control}");
    expect(ui.api.playerDossier).toHaveBeenCalledWith(
      "db-1",
      3,
      expect.any(Function),
    );
    const dialog = await screen.findByRole("dialog", { name: "Dossier" });
    expect(
      await within(dialog).findByText("Dossier: Player 3, A"),
    ).toBeVisible();
    // The bands are worded by the workbench.
    expect(
      within(dialog).getByText(
        "With White: the score is far above expectation (83%).",
      ),
    ).toBeVisible();
    expect(
      within(dialog).getByText("White's wins are short (30 moves)."),
    ).toBeVisible();
    expect(
      within(dialog).getByText(
        "The share of draws is somewhat low (33%), quick draws 33%.",
      ),
    ).toBeVisible();
    // Missing values show as nothing, not "null".
    expect(within(dialog).queryByText(/null/)).toBeNull();
    const medals = within(dialog).getByRole("region", {
      name: "Games with medals",
    });
    expect(within(medals).getByText("Best game")).toBeVisible();

    await ui.user.dblClick(
      within(
        within(dialog).getByRole("region", { name: "Opponents" }),
      ).getByText("Bravo, B"),
    );
    expect(ui.api.statisticsGames).toHaveBeenCalledWith(
      "db-1",
      3,
      { kind: "opponent", id: 5 },
      expect.any(String),
    );
    expect(screen.queryByRole("dialog", { name: "Dossier" })).toBeNull();
  });

  it("loads a listed game", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(screen.getByRole("tab", { name: "Players" }));
    await ui.user.click(
      await screen.findByRole("button", { name: "Choose Player 3, A" }),
    );
    await ui.user.keyboard("{Control>}{Alt>}d{/Alt}{/Control}");
    const dialog = await screen.findByRole("dialog", { name: "Dossier" });
    const mates = await within(dialog).findByRole("region", {
      name: "Mates given",
    });
    await ui.user.dblClick(within(mates).getByText("Bravo, B"));
    expect(ui.api.loadEntry).toHaveBeenCalledWith(
      "db-1",
      "3",
      expect.any(String),
    );
  });
});
