# ChessBase-style workbench shell

Date: 2026-10-12
Status: Implemented on branch `ui-shell` (2026-10-12). Direction agreed in
conversation (layout A, familiar + modern); the user asked to proceed without
per-section review gates.

## Intended outcome

Give Chess Workbench the look, vocabulary and command structure of ChessBase 18
while fixing what the user finds clunky about it. ChessBase users should
recognise the colours, ribbon tabs, command names, tooltips and shortcuts. The
window model is redesigned.

What the user said: port ChessBase's UI using REA; it is clunky; take the
"familiar + modern" direction with layout A; match its colours.

Assumptions: existing features stay as they are (CBH browse, replay, edit, PGN,
UCI analysis). Commands without a backend appear but are disabled and labelled.
Visual assets are recreated, not copied. ChessBase bitmaps, icon PNGs and fonts
never enter the product tree, matching the existing distribution audit.

## Evidence sources (REA and static resources)

| Source | Gives |
|---|---|
| `Messages/*.inc` + `Messages/English/*.strings` | Enum ordinal → `M_*` symbol → English label/tooltip (`M_MN_*`, `M_TX_*`, `M_TAB_*`) |
| Ribbon icon PNG names (`{Theme}_{Command}{size}`) | 358 command identifiers and per-theme icon tint |
| REA/Ghidra on `CBase18.exe` | Ribbon category/panel/button construction, theme defaults, accent colours, keyboard defaults |

Recovered so far (research output in `analysis/ui/`, outside the product tree):

* 806 keyboard-command names (`M_KC_*`), from two name-table initialisers
  (`0x140952560`, frame ids 0–458; `0x14094bb10`, CBase ids 459+).
* 306 default shortcuts, from 12-byte key records written by `0x14094d530` and
  `0x1409488f0`. A key word's high nibble is a modifier mask (Shift 1, Ctrl 2,
  Alt 4), the next nibble is the key class (1 digit, 2 letter, 3 F-key,
  4 arrow, 5 special) and the low byte is the index; e.g. `0x2205` = Ctrl+F.
* Ribbon trees. Builders create tabs (`FUN_140e34f00`), panels
  (`FUN_140e34c70`) and buttons (`FUN_140e34220`/`34260`) with message ids and
  icon names, then attach them with `FUN_140e35a90`. The database window is
  built by `0x14089b170` and the game window by `0x1408ed380`.
* Theme commands `0xa01c`–`0xa01f` (White, Blue, Gray, Black; White first).

Derived research output lives in `analysis/ui/` (outside the product tree).
The product receives only a reviewed, hand-curated command table (names, our own
labels derived from the facts, shortcuts) and colour tokens. Colours and
command names are facts about the interface; they are not copied artwork.

## Layout (A: list + live preview)

```
┌ title/tab strip: [Mega 2025] [Ding – Gukesh]          ⌘K ┐
├ ribbon tabs: Home · Insert · Board · Engine · View · Help ┤
├ ribbon panel: grouped large/small command buttons       ┤
├──────────── database tab ───────────────────────────────┤
│ filter bar (page-only for now)        │  board preview  │
│ game list (virtual, sortable cols)    │  notation strip │
├─────────────────────────────────────────────────────────┤
│ status bar: entries · selection · engine status         │
└─────────────────────────────────────────────────────────┘
```

* Clicking a list row previews the game; double-click or Enter opens the game tab.
* The game tab shows board, notation, and engine panes in resizable splits.
* Up to four databases have a tab each (see [game list](game-list.md)).
  There is one game tab, because the frontend state holds one document.
  Multiple simultaneous game tabs require a multi-document state refactor
  and are follow-up work.
* ChessBase binds Ctrl+K to Add Kibitzer, so command search uses `/` and
  Ctrl+Shift+Space instead of ⌘K.

## Command system

A single typed catalogue (`src/commands/catalogue.ts`) defines each command: id
(ChessBase command identifier), label, tooltip, ribbon tab and group,
shortcut, `when` (enabled predicate) and `run`. The ribbon, ⌘K palette,
keyboard handler and tooltips all read from it. Unimplemented ChessBase commands
are left unbound; the ribbon disables them and the palette lists them as “Not yet
available”.

## Theme

CSS custom properties per theme (`white` default, `blue`, `gray`, `black`),
modelled on ChessBase 18's BCG Office-2016-family styles. Icon tint tokens use
the sampled ChessBase values (white: navy `#2e5583`, blue: crimson `#b2262d`,
gray: teal `#156172`, black: orange `#dc6e3a`). Icons come from an open
outline icon set tinted with the theme token, giving the ChessBase visual
language without its artwork. Board defaults follow the recovered ChessBase 2D
board colours where REA finds them.

## Error handling

Existing diagnostics and unsaved-changes guarding remain unchanged. Disabled
commands explain why in their tooltip. The palette never runs unavailable
commands.

## Testing

Existing behavioural tests continue to pass after selector updates (accessible
names are kept where possible). New tests: registry (unique ids/shortcuts,
availability), palette (search, run, unavailable), ribbon (tab switching, enabled
state), list preview/open flow, theme switching. The UI is also checked visually
in the browser and as a native Tauri build.
