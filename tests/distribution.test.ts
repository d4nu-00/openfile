import { describe, it, expect } from "vitest";
import {
  allowedSourcePath,
  auditCandidatePaths,
} from "../scripts/check-distribution.mjs";
describe("source distribution boundary", () => {
  it("rejects research and proprietary/database material", () => {
    for (const path of [
      "../analysis/classic-sample-games.pgn",
      "tests/fixtures/the test database.cbh",
      "src/extracted.exe",
      "src/foo.CBG",
      "crates/engine/src/private.cba",
      "tests/fixtures/private.2cbh",
      "tests/fixtures/classic-sample-games.pgn",
      "crates/chess-core/src/../../analysis.rs",
      "src\\binary.dll",
      ["", "Users", "example", "private.cbh"].join("/"),
      "C:/private/database.cbh",
      "tooling/reference/game.py",
      "target/release/openfile",
    ])
      expect(allowedSourcePath(path), path).toBe(false);
  });
  it("accepts only authored source, notices and documented synthetic PGN paths", () => {
    for (const path of [
      "crates/cbh-reader/src/header.rs",
      "crates/engine/src/bin/mock-uci.rs",
      "src/components/Board.tsx",
      "src/tests/edit.test.tsx",
      "src-tauri/capabilities/main.json",
      "licenses/cbh2pgn-MIT.txt",
      "tests/fixtures/synthetic-start.pgn",
      "docs/compatibility.md",
      "docs/game-review.md",
      "docs/replay-training.md",
      "docs/media/replay-training-question.jpg",
      "docs/media/replay-training-solved.jpg",
      "docs/media/replay-training-summary.jpg",
      "docs/media/board-drag-flipped.jpg",
      "docs/media/board-setup-drag.jpg",
      "docs/media/board-training-drag.jpg",
      "docs/superpowers/plans/2026-10-19-replay-training-sessions.md",
      "docs/superpowers/specs/2026-10-19-replay-training-sessions-design.md",
      "docs/superpowers/plans/2026-10-18-filtered-opening-tree.md",
      "docs/superpowers/plans/2026-10-18-quick-search-expressions.md",
      "docs/superpowers/specs/2026-10-18-quick-search-expressions-design.md",
      ".github/workflows/desktop.yml",
    ])
      expect(allowedSourcePath(path), path).toBe(true);
    for (const path of [
      "random.txt",
      "src/surprise.zip",
      "tests/fixtures/unknown.pgn",
      "docs/commercial.pgn",
      "docs/superpowers/plans/unreviewed.md",
      "src-tauri/gen/schema.json",
    ])
      expect(allowedSourcePath(path), path).toBe(false);
  });
  it("reports every unsafe candidate instead of overlooking a later file", () => {
    expect(
      auditCandidatePaths([
        "src/App.tsx",
        "private/game.cbh",
        "../analysis/games.pgn",
        "src-tauri/src/lib.rs",
      ]),
    ).toEqual(["private/game.cbh", "../analysis/games.pgn"]);
  });
});
