import { render, screen, waitFor } from "@testing-library/react";
import { it, expect, vi } from "vitest";
import { NotationTree } from "../components/NotationTree";
import type { GameNode } from "../api/types";
it("keeps a 100000-node notation tree bounded and brings selection into its window", async () => {
  const nodes: GameNode[] = Array.from({ length: 100000 }, (_, id) => ({
    id,
    parent: id ? id - 1 : undefined,
    uci: id ? "g1f3" : undefined,
    san: id ? "Nf3" : undefined,
    children: id < 99999 ? [id + 1] : [],
    comments: [],
    nags: [],
  }));
  const onSelect = vi.fn();
  const view = render(
    <NotationTree nodes={nodes} selection={99999} onSelect={onSelect} />,
  );
  await waitFor(() =>
    expect(
      view.container.querySelector('[aria-current="step"]'),
    ).toHaveTextContent("Nf3"),
  );
  expect(screen.getAllByRole("button").length).toBeLessThan(205);
  (
    view.container.querySelector('[aria-current="step"]') as HTMLButtonElement
  ).click();
  expect(onSelect).toHaveBeenCalledWith(99999);
});
