# Chess Workbench

**Your ChessBase databases, on any desktop, free and open source.**

Chess Workbench is a desktop chess database app for macOS, Windows and Linux.
It opens the databases you already have from ChessBase, the classic `.cbh` and
the newer `.2cbh` formats. You can browse, search, analyse and edit them in a
fast, familiar window that looks and handles like ChessBase 18. It is built
with Rust, React and Tauri, works entirely offline, and is licensed
GPL-3.0-or-later.

It is not affiliated with ChessBase GmbH. It reads and writes their file
formats from the user's own files, reimplemented independently. No
ChessBase code, artwork or games ship with it.

## Why use it

- **Open your existing databases.** It reads the classic CBH family and
  ChessBase 18's 2CBH family: games, variations, comments, symbols, arrows
  and coloured squares. It reads players, tournaments, annotators, sources
  and teams too. It was tested against the 11-million-game classic Mega
  Database 2025 and a 2CBH copy of its first 24,783 games.
- **Fast on huge databases.**
  - **Scrolling.** The game list scrolls the whole database, every row.
  - **Sorting.** Sort by any column (player, rating, result, ECO,
    tournament, round, date…) in under a second on 11 million games.
  - **Go to entry.** Jump straight to any game in the current list.
- **ChessBase muscle memory.** The ribbon has ChessBase's tabs, command names
  and default shortcuts, and its White, Blue, Gray and Black themes. Press
  `/` to search every command.
- **Search.** The search mask covers players, colours, wins only, tournament,
  annotator, Elo ranges and differences, year, ECO, length and result.
  - **Quick Filter** searches the whole database by any words.
  - **Find Position** finds every game that reaches the board. An optional
    search booster index makes it much faster.
- **Opening tree and reference search.** Build an opening tree from your own
  games. The Book pane shows games, score, draw rate, average rating,
  performance and the year last played for every move. The Reference pane
  follows your board through the database.
- **Players, tournaments and crosstables.** Every player and tournament is
  listed with game counts and years, and a click opens their games. Round-robin
  and Swiss crosstables show Sonneborn-Berger, Buchholz, performance and FIDE
  category; each result opens its game.
- **Edit and write.**
  - **Editing.** Enter moves and variations, promote and delete lines, add
    comments and symbols, edit game data, set up positions. Up to 100 edits
    can be undone.
  - **Saving.** Save, replace or delete games in `.2cbh` databases.
  - **Several databases.** Keep several open at once, select games with
    Ctrl/Shift-click, and copy and paste them between databases.
- **Engine analysis.** Plug in any UCI engine, such as Stockfish. Configure its
  options and get MultiPV lines with depth, nodes and scores.
- **PGN in and out.** Import PGN collections, and export or copy games as PGN.
- **Private and safe.**
  - **Offline.** It needs no account or network, and nothing is uploaded.
  - **Read-only by default.** A database is only written when you open it
    for editing.
  - **Own folder.** The app keeps its indexes (search booster, opening tree,
    sort orders, name lists) in its own data folder, never beside your
    files.

See the [feature checklist](feature_checklist.md) for what is done and what is
planned, mapped against ChessBase 18's ~180 features.

## Install

### Status

There are no published downloads yet. Today you build it from source (below).

| Platform | Status |
|---|---|
| macOS on Apple Silicon | Built and used daily |
| Windows, Linux | Should build the same way; not yet verified on real machines |

The repository has a CI workflow that builds all three platforms, but it has
not run yet.

### What a release will look like

Tauri, the app framework, produces the native package for each platform. It
has to be built on that platform; cross-compiling is not supported, so a CI
matrix does it.

| Platform | You download | How you install it |
|---|---|---|
| macOS | `Chess Workbench.dmg`, holding `Chess Workbench.app` | Open the `.dmg` and drag the app into Applications. Apple Silicon and Intel need separate builds, or one universal build. |
| Windows | `Chess Workbench_x.y.z_x64-setup.exe` (NSIS) or `.msi` | Run the installer. It uses Microsoft Edge WebView2, which Windows 10 and 11 already have; the installer can fetch it if not. |
| Linux | `.AppImage`, `.deb` or `.rpm` | Make the AppImage executable and run it, or install the package with your package manager. It needs WebKitGTK 4.1. |

**Unsigned builds.** Until the builds are signed, the operating system will
warn the first time you open the app:
- **macOS.** Right-click the app and choose **Open**, or use **System
  Settings › Privacy & Security › Open Anyway**. Signing and notarising
  needs an Apple Developer ID.
- **Windows.** SmartScreen shows "Windows protected your PC". Choose **More
  info › Run anyway**. A code-signing certificate removes this.

### Build from source

You need:
- the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for
  your OS (Xcode command line tools on macOS, WebView2 and the MSVC build
  tools on Windows, WebKitGTK on Linux);
- Rust via rustup (the repository pins 1.99.0);
- Node 24 LTS (`.nvmrc`).

`Cargo.lock` and `package-lock.json` pin every dependency.

```sh
npm ci
npm run tauri -- build
```

That builds the app and every package your platform supports, under
`target/release/bundle/`:
- **macOS:** `macos/Chess Workbench.app` and `dmg/…`;
- **Windows:** `nsis/…-setup.exe` and `msi/…`;
- **Linux:** `appimage/`, `deb/` and `rpm/`.

For just the executable, without installers:

```sh
npm run tauri -- build --no-bundle -- --locked
```

The executable is `target/release/chess-workbench` (`.exe` on Windows). A
database named on the command line opens at start, optionally at an entry:

```sh
chess-workbench "My Games.2cbh" --entry 42
```

For development, `npm run tauri -- dev` runs a debug build with hot reload.
`npm run dev` alone serves the interface in a browser with a demo backend
(`/?demo&view=game&entry=11&node=24&theme=black` opens a ready-made game).
Don't use `tauri dev --release`: release builds only load their bundled pages,
so that window stays white.

## Use

**Opening a database.** Choose **Open Database** (Ctrl+O; Ctrl means ⌘ on
macOS) and pick a `.cbh` or `.2cbh` file. Keep its companion files beside
it with the same name: `.cbg` holds moves, `.cbp`/`.cbt` names, `.cba`
annotations. Anything missing or not understood is reported, never guessed.
Each open database gets a tab.

**The database window** has three lists: **Games**, **Players** and
**Tournaments**.

| To | Do |
|---|---|
| Preview a game | Click it |
| Move through the list | Up/Down |
| Open the game in its own tab | Enter or double-click |
| Sort | Click a column header: ascending, then descending, then back to entry order |
| Select games | Ctrl- and Shift-click, or **Select All** (Ctrl+A) |
| Copy and paste games | **Copy** (Ctrl+C), then **Paste** (Ctrl+V) into a database opened for editing |
| Search | **Filter Games** (Ctrl+F) opens the search mask; **Reset Filter** shows everything again |
| Crosstable | **Cross Table** (T) for the chosen or loaded game's tournament |

**The game window** has the board, the notation and the engine.

| To | Do |
|---|---|
| Replay moves | Left/Right, Home and End |
| Jump to a move | Click it in the notation |
| Add a move | Click a piece, then its destination |
| Edit game data | F2 |
| Add symbols | Annotation palette, Ctrl+Alt+S |
| Undo | Ctrl+Z |
| Flip the board | Ctrl+F |
| Load the next or previous game of the list | F11 / Ctrl+F11 |
| Opening book | **Book** tab (Ctrl+F12) |
| Find the position in the database | **Find Position › Hard Disk** |

**Writing.** Databases are written only in ChessBase 18's `.2cbh` format, and
only when you ask:
- **New** (Ctrl+X) creates a database.
- **Open for Editing** opens an existing `.2cbh` after a warning.
- **Save** (Ctrl+S) appends the game to it, **Replace** (Ctrl+R) overwrites the
  game it came from, and **Delete** marks games deleted.

Classic `.cbh` databases always open read-only. Without an editable database,
Save writes PGN. Unsaved edits are guarded with Save/Discard/Cancel. See
[docs/writing.md](docs/writing.md); ChessBase's Check Integrity rebuilds its
own index files after the workbench writes.

**Engines.** **Add Kibitzer** (Ctrl+K) picks any UCI engine executable. No
engine is bundled.
- **Stockfish.** Homebrew installs it at `/opt/homebrew/bin/stockfish`. In the
  macOS file picker, press ⌘⇧G to type that path.
- **Analysis.** **Infinite Analysis** (Alt+F2) starts and stops it.

**Faster searches and trees** (Maintenance tab):
- **Create Search Booster** speeds up position searches.
- **Create Opening Tree** feeds the Book and Reference panes. It takes about
  ten minutes for the 11-million-game Mega.

See [docs/search.md](docs/search.md), [docs/opening-tree.md](docs/opening-tree.md)
and [docs/game-list.md](docs/game-list.md).

## Not there yet

- Several games open at once in tabs.
- Drawing arrows and squares with the mouse.
- Engine management and several engines at once.
- Tablebases, blunder check and training.
- Reports, printing and publishing.
- Chess960 games.
- ChessBase's online services (cloud databases, Playchess, Let's Check) are
  out of scope. Open services such as Lichess are possible stand-ins.

[Compatibility](docs/compatibility.md) lists exactly what is read, written
and reported as unsupported.

## Verify

```sh
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
npm test
npm run typecheck
npm run build
npm run check:distribution
```

**The source audit.** It checks Git-tracked files against an explicit
allowlist, rejects private or proprietary file families and home paths, and
verifies the locked dependency inventory and notice hashes. Stage new product
files before auditing them. Builds, private validation reports, external
engines, commercial games and extracted application resources never enter the
source. [Validation](docs/validation.md) records what was measured and tested.

After dependency changes, regenerate the notice inventory, review it, then
rerun the audit:

```sh
cargo metadata --format-version 1 --locked > /tmp/workbench-metadata.json
node scripts/generate-notices.mjs --metadata /tmp/workbench-metadata.json --cache /tmp/workbench-license-cache --fetch
```

The optional private corpus comparator needs python-chess, a separately
obtained pinned MIT reference, and explicit external
database/observation/report paths. Proprietary test games and local research
are never product fixtures.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE), the
[third-party notices](THIRD_PARTY_NOTICES.md) and the
[dependency inventory](licenses/dependencies.json). Packages include these
notices. Anyone distributing builds must also offer the corresponding source.
