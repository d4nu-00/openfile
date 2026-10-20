import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { Board } from "../components/Board";

const start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

function rect(board: HTMLElement) {
  board.getBoundingClientRect = () =>
    ({ left: 0, top: 0, width: 800, height: 800 }) as DOMRect;
}

function pointer(square: string, flipped = false) {
  const col = (flipped ? "hgfedcba" : "abcdefgh").indexOf(square[0]);
  const row = (flipped ? "12345678" : "87654321").indexOf(square[1]);
  return { clientX: col * 100 + 50, clientY: row * 100 + 50 };
}

it("drags a piece to a legal square with live destinations and no follow-up click", () => {
  const onMove = vi.fn();
  const onSquare = vi.fn();
  render(
    <Board
      fen={start}
      flipped={false}
      legalUci={["e2e3", "e2e4"]}
      onMove={onMove}
      onSquare={onSquare}
    />,
  );
  const board = screen.getByRole("grid", { name: "Chess board" });
  rect(board);
  const e2 = screen.getByRole("button", { name: "e2 White pawn" });
  fireEvent.pointerDown(e2, {
    button: 0,
    pointerId: 1,
    pointerType: "mouse",
    ...pointer("e2"),
  });
  fireEvent.pointerMove(board, { pointerId: 1, ...pointer("e4") });
  expect(screen.getByRole("button", { name: "e4 empty" })).toHaveAttribute(
    "data-legal",
    "true",
  );
  expect(board).toHaveAttribute("data-dragging", "e2");
  fireEvent.pointerUp(board, { pointerId: 1, ...pointer("e4") });
  fireEvent.click(e2);
  expect(onMove).toHaveBeenCalledOnce();
  expect(onMove).toHaveBeenCalledWith("e2", "e4");
  expect(onSquare).not.toHaveBeenCalled();
});

it("rejects illegal drops and cancels on pointer cancellation", () => {
  const onMove = vi.fn();
  render(
    <Board fen={start} flipped={false} legalUci={["e2e4"]} onMove={onMove} />,
  );
  const board = screen.getByRole("grid", { name: "Chess board" });
  rect(board);
  const e2 = screen.getByRole("button", { name: "e2 White pawn" });
  fireEvent.pointerDown(e2, { button: 0, pointerId: 2, ...pointer("e2") });
  fireEvent.pointerMove(board, { pointerId: 2, ...pointer("e5") });
  fireEvent.pointerUp(board, { pointerId: 2, ...pointer("e5") });
  fireEvent.pointerDown(e2, { button: 0, pointerId: 3, ...pointer("e2") });
  fireEvent.pointerMove(board, { pointerId: 3, ...pointer("e4") });
  fireEvent.pointerCancel(board, { pointerId: 3 });
  expect(board).not.toHaveAttribute("data-dragging");
  expect(onMove).not.toHaveBeenCalled();
});

it("maps a flipped Chess960 king-to-rook drop to its legal move", () => {
  const onMove = vi.fn();
  render(
    <Board
      fen="1k6/8/8/8/8/8/8/RK5R w HA - 0 1"
      flipped
      legalUci={["b1h1", "b1a1"]}
      onMove={onMove}
    />,
  );
  const board = screen.getByRole("grid", { name: "Chess board" });
  rect(board);
  fireEvent.pointerDown(screen.getByRole("button", { name: "b1 White king" }), {
    button: 0,
    pointerId: 4,
    ...pointer("b1", true),
  });
  fireEvent.pointerMove(board, { pointerId: 4, ...pointer("h1", true) });
  expect(screen.getByRole("button", { name: "h1 White rook" })).toHaveAttribute(
    "data-legal",
    "true",
  );
  fireEvent.pointerUp(board, { pointerId: 4, ...pointer("h1", true) });
  expect(onMove).toHaveBeenCalledWith("b1", "h1");
});

it("keeps Alt drawing separate from piece dragging", () => {
  const onMove = vi.fn();
  const onMark = vi.fn();
  render(
    <Board
      fen={start}
      flipped={false}
      legalUci={["g1f3"]}
      onMove={onMove}
      onMark={onMark}
    />,
  );
  const board = screen.getByRole("grid", { name: "Chess board" });
  rect(board);
  fireEvent.pointerDown(
    screen.getByRole("button", { name: "g1 White knight" }),
    { button: 0, altKey: true, pointerId: 5, ...pointer("g1") },
  );
  fireEvent.pointerMove(board, { pointerId: 5, ...pointer("f3") });
  fireEvent.pointerUp(board, { pointerId: 5, ...pointer("f3") });
  expect(onMark).toHaveBeenCalledWith({ from: "g1", to: "f3", color: "G" });
  expect(onMove).not.toHaveBeenCalled();
});

it("preserves click-to-move by capturing only once a drag begins", () => {
  const onSquare = vi.fn();
  render(
    <Board
      fen={start}
      flipped={false}
      legalUci={["e2e4"]}
      onMove={() => {}}
      onSquare={onSquare}
    />,
  );
  const board = screen.getByRole("grid", { name: "Chess board" });
  rect(board);
  const capture = vi.fn();
  board.setPointerCapture = capture;
  const e2 = screen.getByRole("button", { name: "e2 White pawn" });
  fireEvent.pointerDown(e2, { button: 0, pointerId: 6, ...pointer("e2") });
  expect(capture).not.toHaveBeenCalled();
  fireEvent.pointerUp(e2, { pointerId: 6, ...pointer("e2") });
  fireEvent.click(e2);
  expect(onSquare).toHaveBeenCalledWith("e2");
  fireEvent.pointerDown(e2, { button: 0, pointerId: 7, ...pointer("e2") });
  fireEvent.pointerMove(board, { pointerId: 7, ...pointer("e4") });
  expect(capture).toHaveBeenCalledWith(7);
});

it("does not turn a completed drag into a click when pointer capture releases", () => {
  const onSquare = vi.fn();
  const onMove = vi.fn();
  render(
    <Board
      fen={start}
      flipped={false}
      legalUci={["e2e4"]}
      onMove={onMove}
      onSquare={onSquare}
    />,
  );
  const board = screen.getByRole("grid", { name: "Chess board" });
  rect(board);
  const e2 = screen.getByRole("button", { name: "e2 White pawn" });
  fireEvent.pointerDown(e2, { button: 0, pointerId: 9, ...pointer("e2") });
  fireEvent.pointerMove(board, { pointerId: 9, ...pointer("e4") });
  fireEvent.pointerUp(board, { pointerId: 9, ...pointer("e4") });
  fireEvent.lostPointerCapture(board, { pointerId: 9 });
  fireEvent.click(e2);
  expect(onMove).toHaveBeenCalledWith("e2", "e4");
  expect(onSquare).not.toHaveBeenCalled();
});

it("cancels a drag on Escape or release outside the board", () => {
  const onMove = vi.fn();
  render(
    <Board fen={start} flipped={false} legalUci={["e2e4"]} onMove={onMove} />,
  );
  const board = screen.getByRole("grid", { name: "Chess board" });
  rect(board);
  const e2 = screen.getByRole("button", { name: "e2 White pawn" });
  fireEvent.pointerDown(e2, { button: 0, pointerId: 10, ...pointer("e2") });
  fireEvent.pointerMove(board, { pointerId: 10, ...pointer("e4") });
  fireEvent.keyDown(window, { key: "Escape" });
  fireEvent.pointerUp(board, { pointerId: 10, ...pointer("e4") });
  expect(board).not.toHaveAttribute("data-dragging");
  fireEvent.pointerDown(e2, { button: 0, pointerId: 11, ...pointer("e2") });
  fireEvent.pointerMove(board, { pointerId: 11, ...pointer("e4") });
  fireEvent.pointerUp(board, { pointerId: 11, clientX: 900, clientY: 350 });
  expect(onMove).not.toHaveBeenCalled();
});
