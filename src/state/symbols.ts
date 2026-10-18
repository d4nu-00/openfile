/**
 * The reference app's annotation symbols, as PGN NAG numbers in the three slots a
 * The reference app symbol annotation has: move, position and prefix. Each slot
 * holds one symbol, so choosing one replaces the slot's previous symbol.
 *
 * The groups and their order are those of the popups the reference app opens when
 * "!" or "?", "=" and "$" are typed in a game window (the reference binary:
 * `0x140a65f60`, `0x140a65aa0`, `0x140a66690`; see docs/game-window.md).
 */
export type SymbolSlot = "move" | "position" | "prefix";

export interface SymbolChoice {
  nag: number;
  name: string;
}

export const SYMBOL_GROUPS: Record<SymbolSlot, SymbolChoice[]> = {
  move: [
    { nag: 1, name: "Good move" },
    { nag: 2, name: "Mistake" },
    { nag: 5, name: "Interesting move" },
    { nag: 6, name: "Dubious move" },
    { nag: 3, name: "Brilliant move" },
    { nag: 4, name: "Blunder" },
    { nag: 22, name: "Zugzwang" },
    { nag: 8, name: "Only move" },
  ],
  position: [
    { nag: 18, name: "White is winning" },
    { nag: 16, name: "White is better" },
    { nag: 14, name: "White is slightly better" },
    { nag: 11, name: "Equal" },
    { nag: 15, name: "Black is slightly better" },
    { nag: 17, name: "Black is better" },
    { nag: 19, name: "Black is winning" },
    { nag: 13, name: "Unclear" },
    { nag: 44, name: "With compensation" },
    { nag: 40, name: "With attack" },
    { nag: 36, name: "Initiative" },
    { nag: 132, name: "Counterplay" },
    { nag: 138, name: "Zeitnot" },
    { nag: 32, name: "Development advantage" },
    { nag: 146, name: "Novelty" },
  ],
  prefix: [
    { nag: 145, name: "Editorial annotation" },
    { nag: 142, name: "Better is" },
    { nag: 143, name: "Worse is" },
    { nag: 144, name: "Equivalent is" },
    { nag: 140, name: "With the idea" },
    { nag: 141, name: "Aimed against" },
  ],
};

/** The slot a NAG is stored in, as the 2CBH writer stores it. */
export function slotOf(nag: number): SymbolSlot {
  if ((nag >= 1 && nag <= 9) || nag === 22) return "move";
  if (nag >= 140 && nag <= 145) return "prefix";
  return "position";
}

export function symbolName(nag: number): string | undefined {
  for (const group of Object.values(SYMBOL_GROUPS)) {
    const found = group.find((c) => c.nag === nag);
    if (found) return found.name;
  }
  return undefined;
}

/**
 * The symbols a move has after choosing `nag` in `slot`: the slot's old
 * symbol is replaced; `undefined` clears the slot.
 */
export function chooseSymbol(
  nags: number[],
  slot: SymbolSlot,
  nag: number | undefined,
): number[] {
  const kept = nags.filter((n) => slotOf(n) !== slot);
  return nag === undefined ? kept : [...kept, nag];
}

/** The symbols after toggling `nag`: set it in its slot, or remove it. */
export function toggleNag(nags: number[], nag: number): number[] {
  if (nags.includes(nag)) return nags.filter((n) => n !== nag);
  return chooseSymbol(nags, slotOf(nag), nag);
}
