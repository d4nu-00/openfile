import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWorkbench } from "./harness";

describe("prepare against a player", () => {
  it("prepares with White, switches colour and lists a line's games", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(screen.getByRole("tab", { name: "Players" }));
    await ui.user.click(
      await screen.findByRole("button", { name: "Choose Player 3, A" }),
    );
    await ui.user.keyboard("{Control>}w{/Control}");
    expect(ui.api.prepareAgainst).toHaveBeenCalledWith(
      "db-1",
      3,
      true,
      expect.any(Function),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Prepare against White",
    });
    expect(
      await within(dialog).findByText("Prepare against White: Player 3, A"),
    ).toBeVisible();
    const lines = within(dialog).getByRole("region", { name: "Repertoire" });
    expect(within(lines).getByText("1.e4 c5 2.Nf3")).toBeVisible();
    expect(within(lines).getByText("75%")).toBeVisible();
    expect(within(dialog).getByText("2450")).toBeVisible();

    await ui.user.click(
      within(dialog).getByRole("button", { name: "As Black" }),
    );
    expect(ui.api.prepareAgainst).toHaveBeenLastCalledWith(
      "db-1",
      3,
      false,
      expect.any(Function),
    );
    const black = await screen.findByRole("dialog", {
      name: "Prepare against Black",
    });
    await ui.user.dblClick(within(black).getByText("1.e4 c5 2.Nf3"));
    expect(ui.api.preparationGames).toHaveBeenCalledWith(
      "db-1",
      3,
      false,
      {
        startFen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        moves: ["e2e4", "c7c5", "g1f3"],
      },
      expect.any(String),
    );
    expect(
      await screen.findByText("Player 3, A as Black: 1.e4 c5 2.Nf3: 2 games"),
    ).toBeVisible();
    expect(
      screen.queryByRole("dialog", { name: /Prepare against/ }),
    ).toBeNull();
  });

  it("shows the preparation in the Book until the database tree is asked for", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(
      await screen.findByRole("button", { name: "Load entry 3" }),
    );
    await waitFor(() => expect(ui.api.loadEntry).toHaveBeenCalled());
    await ui.user.click(screen.getByRole("tab", { name: "Players" }));
    await ui.user.click(
      await screen.findByRole("button", { name: "Choose Player 3, A" }),
    );
    await ui.user.keyboard("{Control>}b{/Control}");
    const dialog = await screen.findByRole("dialog", {
      name: "Prepare against Black",
    });
    await ui.user.click(
      await within(dialog).findByRole("button", { name: "Show in Book" }),
    );
    const book = await screen.findByLabelText("Opening book");
    expect(
      await within(book).findByText("Prepare against Black: Player 3, A"),
    ).toBeVisible();
    await waitFor(() => expect(ui.api.preparedTree).toHaveBeenCalled());
    expect(await within(book).findByText("e4")).toBeVisible();

    await ui.user.click(
      within(book).getByRole("button", { name: "Database tree" }),
    );
    expect(ui.api.closePreparation).toHaveBeenCalledWith("db-1");
    expect(within(book).queryByText(/Prepare against/)).toBeNull();
  });

  it("Prepare Against opens the Players list", async () => {
    const ui = renderWorkbench({ initialView: "database" });
    await ui.user.click(
      await screen.findByRole("button", { name: "Prepare Against" }),
    );
    expect(screen.getByRole("tab", { name: "Players" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(ui.api.prepareAgainst).not.toHaveBeenCalled();
  });
});
