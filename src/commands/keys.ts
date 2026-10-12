/** ChessBase-style shortcut strings ("Ctrl+Shift+K", "Alt+F2", "+"). */
export const isMac =
  typeof navigator !== "undefined" &&
  /Mac|iPhone|iPad/.test(navigator.platform);

const named: Record<string, string> = {
  Up: "ArrowUp",
  Down: "ArrowDown",
  Left: "ArrowLeft",
  Right: "ArrowRight",
  Space: " ",
};

function split(shortcut: string) {
  // "+" alone and "Ctrl++" both name the plus key.
  const parts = shortcut.endsWith("++")
    ? [...shortcut.slice(0, -2).split("+"), "+"]
    : shortcut === "+"
      ? ["+"]
      : shortcut.split("+");
  const key = parts.pop()!;
  return { mods: new Set(parts), key };
}

export function matches(shortcut: string, event: KeyboardEvent) {
  const { mods, key } = split(shortcut);
  // ChessBase's Ctrl is the platform command key; accept either on all hosts.
  const ctrl = event.ctrlKey || event.metaKey;
  if (mods.has("Ctrl") !== ctrl) return false;
  if (mods.has("Alt") !== event.altKey) return false;
  const printable = key.length === 1 && !/[A-Z0-9]/i.test(key);
  if (!printable && mods.has("Shift") !== event.shiftKey) return false;
  const expected = named[key] ?? key;
  if (/^[A-Z]$/i.test(expected))
    return event.code === `Key${expected.toUpperCase()}`;
  if (/^[0-9]$/.test(expected)) return event.code === `Digit${expected}`;
  return event.key === expected;
}

export function format(shortcut: string) {
  const { mods, key } = split(shortcut);
  if (!isMac) return shortcut;
  const symbols = [
    mods.has("Ctrl") ? "⌘" : "",
    mods.has("Alt") ? "⌥" : "",
    mods.has("Shift") ? "⇧" : "",
  ].join("");
  const arrows: Record<string, string> = {
    Up: "↑",
    Down: "↓",
    Left: "←",
    Right: "→",
  };
  return symbols + (arrows[key] ?? key);
}
