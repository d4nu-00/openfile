import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/target/**"] },
  },
  build: { target: "safari15" },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
    // Bound concurrent DOM environments on local machines and native CI runners.
    maxWorkers: 2,
    // Full workbench interaction tests can exceed Vitest's 5s default on Windows CI.
    testTimeout: 15_000,
  },
});
