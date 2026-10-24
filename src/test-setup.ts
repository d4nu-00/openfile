import "@testing-library/jest-dom/vitest";
import { cleanup, configure } from "@testing-library/react";
import { afterEach } from "vitest";
afterEach(cleanup);
// findBy*/waitFor give up after 1s by default, which slow Windows CI runners
// overrun mid-flow; testTimeout in vite.config.ts still bounds each test.
configure({ asyncUtilTimeout: 5000 });
// jsdom lacks ResizeObserver, which the resizable panes observe.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;
// jsdom gives every element a zero rect at the origin, so the split-pane hit
// test would treat each click as a separator drag. Keep separators off-screen.
const rect = Element.prototype.getBoundingClientRect;
Element.prototype.getBoundingClientRect = function () {
  if (this.hasAttribute("data-separator"))
    return DOMRect.fromRect({ x: -10000, y: -10000, width: 4, height: 4 });
  return rect.call(this);
};
