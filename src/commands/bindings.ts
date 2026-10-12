import { useEffect, useRef } from "react";
import { commandList, type ViewId } from "./catalogue";
import { matches } from "./keys";

/** What the running app can do for a command. Unbound commands are shown as unavailable. */
export interface Binding {
  run: () => unknown;
  enabled?: boolean;
  pressed?: boolean;
  label?: string;
}
export type Bindings = Partial<Record<string, Binding>>;

const editable =
  'input,textarea,select,[contenteditable="true"],[role="separator"]';

/** Dispatch ChessBase default shortcuts for the active view. */
export function useShortcuts(
  view: ViewId,
  bindings: Bindings,
  paused: boolean,
) {
  const latest = useRef({ view, bindings, paused });
  latest.current = { view, bindings, paused };
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const { view, bindings, paused } = latest.current;
      if (paused || event.defaultPrevented) return;
      const inField = (event.target as HTMLElement).closest?.(editable);
      for (const command of commandList) {
        if (!command.keys || !command.views.includes(view)) continue;
        // In text fields only modified shortcuts apply, so typing stays intact.
        if (inField && !/Ctrl|Alt|^F\d/.test(command.keys)) continue;
        if (
          inField &&
          /^Ctrl\+[A-Z]$/.test(command.keys) &&
          "ACVXYZ".includes(command.keys.slice(-1))
        )
          continue;
        if (!matches(command.keys, event)) continue;
        const binding = bindings[command.id];
        if (!binding || binding.enabled === false) continue;
        event.preventDefault();
        void binding.run();
        return;
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
