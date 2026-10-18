import { useEffect, useRef, useState } from "react";
import type { GameNode } from "../api/types";
import { nagSymbols } from "./NotationTree";
import {
  SYMBOL_GROUPS,
  chooseSymbol,
  slotOf,
  type SymbolSlot,
} from "../state/symbols";

const TITLES: Record<SymbolSlot, string> = {
  move: "Move symbol",
  position: "Evaluation",
  prefix: "Prefix symbol",
};

/**
 * The popup the reference app shows when a symbol key is typed in the game window:
 * "!" or "?" for move symbols, "=" for evaluations, "$" for prefixes. The
 * first entry, "(none)", clears the group. Arrow keys and Enter choose,
 * Escape closes.
 */
export function SymbolMenu({
  slot,
  node,
  onChoose,
  onClose,
}: {
  slot: SymbolSlot;
  node: GameNode;
  onChoose: (nags: number[]) => void;
  onClose: () => void;
}) {
  const choices = [undefined, ...SYMBOL_GROUPS[slot].map((c) => c.nag)];
  const current = node.nags.find((n) => slotOf(n) === slot);
  const [active, setActive] = useState(() =>
    Math.max(0, choices.indexOf(current)),
  );
  const list = useRef<HTMLUListElement>(null);
  useEffect(() => list.current?.focus(), []);
  const choose = (nag: number | undefined) =>
    onChoose(chooseSymbol(node.nags, slot, nag));
  function key(event: React.KeyboardEvent) {
    const last = choices.length - 1;
    const step: Record<string, number> = {
      ArrowDown: active === last ? 0 : active + 1,
      ArrowUp: active === 0 ? last : active - 1,
      Home: 0,
      End: last,
    };
    if (event.key in step) setActive(step[event.key]);
    else if (event.key === "Enter" || event.key === " ")
      choose(choices[active]);
    else if (event.key === "Escape") onClose();
    else return;
    event.preventDefault();
    event.stopPropagation();
  }
  return (
    <div className="symbol-menu" role="dialog" aria-label={TITLES[slot]}>
      <ul
        ref={list}
        role="listbox"
        tabIndex={0}
        aria-label={TITLES[slot]}
        aria-activedescendant={`symbol-${slot}-${active}`}
        onKeyDown={key}
        onBlur={onClose}
      >
        {choices.map((nag, i) => {
          const name =
            nag === undefined
              ? "(none)"
              : SYMBOL_GROUPS[slot].find((c) => c.nag === nag)!.name;
          return (
            <li
              key={nag ?? "none"}
              id={`symbol-${slot}-${i}`}
              role="option"
              aria-selected={i === active}
              aria-label={name}
              className={nag === current ? "current" : undefined}
              onMouseEnter={() => setActive(i)}
              // Choose before the list loses focus and closes.
              onMouseDown={(e) => {
                e.preventDefault();
                choose(nag);
              }}
            >
              <span className="symbol">
                {nag === undefined ? "" : nagSymbols[nag]}
              </span>
              <span>{name}</span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
