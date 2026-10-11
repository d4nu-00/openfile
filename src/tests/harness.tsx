import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { vi } from "vitest";
import { App } from "../App";
import { backend } from "../api/client";
import type {
  GameResponse,
  PositionResponse,
  EntrySummary,
} from "../api/types";
export const startFen =
  "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
export const summary: EntrySummary = {
  ordinal: "1",
  kind: "game",
  deleted: false,
  white: "Alpha",
  black: "Beta",
  event: "Synthetic",
  result: "*",
  diagnostics: [],
};
export const selected: GameResponse = {
  requestId: "initial",
  documentId: "doc-1",
  summary,
  metadataStatus: "available",
  movesStatus: "available",
  annotationsStatus: "notDecoded",
  diagnostics: [],
  game: {
    startFen,
    tags: { White: "Alpha", Black: "Beta", Result: "*" },
    nodes: [
      { id: 0, children: [1], comments: [], nags: [] },
      {
        id: 1,
        parent: 0,
        uci: "e2e4",
        san: "e4",
        children: [2, 3],
        comments: ["Centre"],
        nags: [1],
      },
      {
        id: 2,
        parent: 1,
        uci: "e7e5",
        san: "e5",
        children: [],
        comments: [],
        nags: [],
      },
      {
        id: 3,
        parent: 1,
        uci: "c7c5",
        san: "c5",
        children: [],
        comments: [],
        nags: [],
      },
    ],
  },
};
export const database = {
  requestId: "initial",
  databaseId: "db-1",
  entryCount: "401",
  companions: [],
  diagnostics: [],
};
export const page = {
  requestId: "initial",
  entries: [
    summary,
    { ...summary, ordinal: "2", deleted: true },
    { ...summary, ordinal: "3", kind: "text" as const },
  ],
  next: "201",
};
export function mockApi() {
  return {
    ...backend,
    chooseDatabase: vi.fn().mockResolvedValue("/synthetic/test.cbh"),
    openDatabase: vi
      .fn()
      .mockImplementation(async (_path, requestId) => ({
        ...database,
        requestId,
      })),
    readPage: vi
      .fn()
      .mockImplementation(async (_db, start, _limit, requestId) => ({
        ...page,
        requestId,
        next: start === "1" ? "201" : undefined,
        entries:
          start === "1" ? page.entries : [{ ...summary, ordinal: start }],
      })),
    loadEntry: vi
      .fn()
      .mockImplementation(async (_db, ordinal, requestId) => ({
        ...selected,
        summary: { ...summary, ordinal },
        requestId,
      })),
    positionAt: vi
      .fn()
      .mockImplementation(
        async (documentId, nodeId, requestId): Promise<PositionResponse> => ({
          requestId,
          documentId,
          nodeId,
          fen:
            nodeId === 0
              ? startFen
              : "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
          turn: nodeId === 0 ? "white" : "black",
          legalUci: nodeId === 0 ? ["e2e4", "d2d4"] : ["e7e5", "c7c5"],
        }),
      ),
    closeDatabase: vi.fn().mockResolvedValue(undefined),
    closeDocument: vi.fn().mockResolvedValue(undefined),
  };
}
export function renderWorkbench({
  dirty = false,
  clean = false,
}: { dirty?: boolean; clean?: boolean } = {}) {
  const api = mockApi();
  const user = userEvent.setup();
  const view = render(
    <App
      api={api}
      initialState={
        clean
          ? undefined
          : {
              database,
              page,
              document: selected,
              position: {
                requestId: "initial",
                documentId: "doc-1",
                nodeId: 0,
                fen: startFen,
                turn: "white",
                legalUci: ["e2e4", "d2d4"],
              },
              dirty,
            }
      }
    />,
  );
  return { ...view, api, user };
}
