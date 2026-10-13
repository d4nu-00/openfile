import { createRoot } from "react-dom/client";
import { isTauri } from "@tauri-apps/api/core";
import { App } from "./App";
import type { WorkbenchApi } from "./api/client";
import type { WorkbenchState } from "./state/workbench";

async function start() {
  let api: WorkbenchApi | undefined;
  let initialState: WorkbenchState | undefined;
  let initialView: "database" | "game" | undefined;
  // Plain-browser development previews use a demo backend; Tauri always uses the real one.
  if (import.meta.env.DEV && !isTauri()) {
    api = (await import("./dev/demoApi")).demoApi();
    // ?demo&view=game&entry=11&node=33&theme=black opens a ready-made preview.
    const q = new URLSearchParams(location.search);
    if (q.has("theme"))
      localStorage.setItem("workbench.theme", q.get("theme")!);
    if (q.has("demo")) {
      const database = await api.openDatabase("/demo", "demo");
      const document = await api.loadEntry(
        database.databaseId,
        q.get("entry") ?? "1",
        "demo",
      );
      const position = await api.positionAt(
        document.documentId!,
        Number(q.get("node") ?? 0),
        "demo",
      );
      initialState = { database, document, position };
      initialView = q.get("view") === "game" ? "game" : "database";
    }
  }
  createRoot(document.getElementById("root")!).render(
    <App api={api} initialState={initialState} initialView={initialView} />,
  );
}
void start();
