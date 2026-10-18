# Cross-platform chess database workbench

Date: 2026-10-09  
Status: Approved by the user on 2026-10-09; implementation has not started.

## Intended outcome

Build an independent, open-source desktop application for Windows, macOS and
Linux that covers the practical chess database workflows of ChessBase. The
user's acceptance priority is reading existing `.cbh` databases. Feature parity
is the overall direction; the first deliverable is a usable database workbench,
followed by separately scoped search, preparation and training subsystems.

The first deliverable must open a real classic CBH database, show its game list,
replay supported games with variations, export PGN and analyse positions with a
local UCI engine. Merely listing headers or displaying a demonstration board
does not meet this requirement. The supplied Mega Database is the private local
compatibility target, including its scale of more than eleven million entries.

Assumptions: local desktop use comes first; offline access is essential; the
original database is kept read-only; user changes are saved as PGN in this first
stage. The app is named OpenFile (first called “Chess Workbench”); naming is not a dependency for
implementation. GPL-3.0-or-later is the proposed project license, subject to an
explicit dependency and attribution check before publishing source.

## Approach

| Approach | Benefit | Cost |
|---|---|---|
| Rust core with Tauri and React/TypeScript | One native database/engine layer, shared desktop UI, typed boundaries, direct bounded file access | Requires a Rust toolchain and platform-specific packaging validation |
| Python with Qt | Fast reuse of the existing experimental decoder and chess tooling | Larger runtime distribution; database performance and packaging need extra attention |
| Electron with TypeScript and a native parser | Mature desktop UI tooling | Larger bundled runtime and another native integration boundary |

Use Rust, Tauri 2 and React/TypeScript, as previously proposed. Separate the
format reader from the desktop shell so it can be tested and used by a future
CLI without a webview. Python investigation scripts remain comparison tools;
they are not required at application runtime.

## First deliverable

1. Open a classic CBH file through the native file picker. Discover its companion
   files by basename and extension, handling extension case differences.
2. Page through entries with White, Black, event, date, result, ratings and ECO
   where understood. Mark deleted entries and guiding texts distinctly.
3. Select a supported game and show an interactive board and notation tree.
   Navigate to the start/end, step moves, select branches and flip the board.
4. Create an editable working copy of the game. Add legal moves, variations and
   text comments, then save/export PGN using a destination selected by the user.
5. Open PGN for the same replay/edit workflow. Export the selected CBH game,
   retaining decoded variations and metadata and reporting omitted source data.
6. Choose a local UCI engine executable, start/stop analysis of the current
   position, and display principal variations, depth, score and nodes. Support
   engine discovery through `uci` and `isready`, including configurable options.

Original CBH files are never rewritten. Bulk import, full-database search and
bulk export are subsequent work; opening a large database does not require
converting or copying the entire corpus.

## CBH compatibility contract

CBH is a file family. `.cbh` stores entry headers; `.cbg` supplies moves; `.cbp`
and `.cbt` supply player and tournament names. Optional files supply annotations,
sources and other data. A lone CBH file may permit a header listing but cannot
promise complete game replay. Missing companions produce actionable messages
and explicit partial-open status, rather than an unexplained empty database.

The first reader targets classic 46-byte CBH records and standard-chess CBG
encodings supported by the audited MIT-licensed `cbh2pgn` reference. Port its
relevant decoding logic with attribution, and verify against independently
specified chess positions and expected move trees. Include supported noninitial
positions after confirming their layout and validating legal replay.

Every entry has separate metadata, move and annotation status. An unsupported
encoding or corrupt stream must be reported for that entry without crashing
the database browser or fabricating a valid game. Deleted entries remain
identifiable; guiding texts appear as text entries whose content support is
reported separately. Unknown result, date, encoding or ECO values remain
unknown, rather than being converted to a plausible value.

Existing research validates bounded standard-game samples. It does not prove
all classic encodings, Chess960, text content or CBA annotations. Until their
readers are implemented, the UI and PGN export explicitly identify unavailable
annotations and other undecoded content. User-created comments work in PGN;
they do not imply that original CBA comments were imported.

`.2cbh` is a later adapter behind the same interface. Its standard moves have
experimental validation, but its name libraries and annotation semantics need
more work. It is not advertised as supported by the first release.

## Components and boundaries

The future product lives in `app/`, independently of extracted material and
analysis artifacts. Only authored source, permitted dependencies, attribution,
build configuration and synthetic test fixtures enter the publishable project.

| Component | Responsibility | Interface |
|---|---|---|
| `chess-core` | Positions, legal moves, SAN, game trees and PGN conversion | Typed positions and move-tree values; no filesystem or GUI dependencies |
| `cbh-reader` | Checked header/entity access, standard CBG decoding, compatibility diagnostics | Open database, read entry page, load selected game; no UI or engine dependencies |
| `engine` | UCI process lifecycle, parsing and cancellable analysis | Engine options and analysis events; uses positions from `chess-core` |
| Desktop backend | File selection, database handles, background jobs, save/export | Narrow typed Tauri commands and events |
| Desktop UI | Database table, board, notation, editing state and analysis pane | Backend commands; no direct parsing of binary files |

An open operation returns a database handle, entry count, available companions
and compatibility warnings. Page requests use the handle, ordinal and bounded
limit. Game loading returns header fields, a starting position, move tree and
structured diagnostics. Database IDs and 64-bit offsets cross the JavaScript
boundary as strings or opaque handles to avoid loss of integer precision.

The move tree records each move, its ordered continuations, comments and NAGs,
including a distinguished main continuation. The UI tracks selection by stable
node ID, not by displayed notation text. Unsupported binary content remains in
the original source files; application diagnostics record its location without
claiming a lossless PGN conversion.

## Data flow and performance

Opening reads and validates headers and companion availability. Checked
positional reads fetch only the requested entry page. Entity lookups use bounded
caches, and game loading fetches only the selected move stream. Checked
arithmetic and file-length checks precede allocations and reads.

File decoding runs away from the UI thread. Requests carry generation IDs so a
slow earlier game selection cannot replace a newer selection. Page size is
capped at 200 entries; entity caching has a fixed memory budget. Per-game byte,
move-node and variation-depth limits return explicit limit diagnostics.

The first performance checks target first-page display within two seconds and
ordinary sampled-game load within one second on this Mac with local storage,
after application startup. These are measurement targets, not established
results. Opening must avoid a complete corpus scan and memory use must not grow
proportionally to database size. Later search uses cancellable background
indexing with a separate cache; indexing is not part of this initial design.

## UI and engine behaviour

Use a desktop layout with a database table, board, notation tree and resizable
analysis pane. Display database/entry compatibility next to relevant actions.
The selected move highlights both the board position and notation node.
Navigation, file opening and saving have keyboard actions; board coordinates,
focus indicators and move selection remain legible when the board is flipped.

Changing an edited game prompts save/discard/cancel. PGN writes use a temporary
file in the destination directory and replacement only after successful output.
Opening or decoding failures do not destroy the current working game.

An engine is explicitly selected by the user and started with structured
arguments without a shell. The backend completes the UCI handshake before
analysis, applies supported options, and sends the position including its move
history. Position changes stop earlier analysis before restarting. Scores retain
their side-to-move interpretation internally and the UI labels its perspective.
MultiPV lines update incrementally. Engine exit, handshake timeout and invalid
output produce recoverable errors; close/stop cleans up the child process.
No engine is bundled initially.

## Verification and release acceptance

Use synthetic fixtures authored for the project to cover headers, endianness,
entity names, initial/setup positions, castling, promotion, en passant, captures,
nested variations and malformed or truncated input. Their expected chess
positions and trees must be specified independently of the decoder under test.
Unknown modes, missing companions, oversized lengths and deeply nested streams
must produce deterministic errors, not panic or unbounded allocation.

PGN round trips must preserve supported game trees and editing semantics.
A scripted mock UCI process exercises handshake, option parsing, stop/restart,
score perspective and shutdown. A real locally installed engine supplies a
separate manual integration check. UI tests cover a complete open/select/replay/
branch/edit/export workflow and stale-selection handling.

An opt-in local corpus check compares the Rust decoder against the existing
classic reference on the 36 overlapping games already investigated, then a
larger deterministic bounded sample. Record decoded, unsupported and failed
entries separately. Neither proprietary binary assets nor commercial database
contents are included as public fixtures or uploaded by CI.

Build and exercise the app on macOS locally. Add native Windows, macOS and Linux
CI jobs and synthetic-fixture checks. A configured CI workflow alone is not
evidence of cross-platform success. Release compatibility claims name tested
systems and architectures. Signing and installer distribution follow after
functional builds; they are not prerequisites for a locally runnable app.

The first deliverable is accepted only when the real classic database can be
paged, sampled supported games and their variations can be replayed and exported,
a working game can be edited and saved as PGN, UCI analysis works, compatibility
limitations are visible, and all three desktop builds have actually passed.

## Follow-on work toward equivalent functionality

1. Broaden format fidelity: CBA comments/NAGs, guiding text, special encodings,
   newer database/name libraries and explicit import coverage reports.
2. Add cancellable metadata and position search, material/manoeuvre filters,
   duplicate detection, statistics and opening trees over persistent indexes.
3. Add repertoire management, preparation reports, batch engine analysis,
   tablebases and training workflows.
4. Add publishing and multimedia tools; design any online features around
   independently available services and protocols.

Each stage gets its own scoped design and verification. Exact proprietary UI,
embedded assets and hosted ChessBase services are not dependencies of the
independent product. A full feature-equivalence claim requires workflow-by-
workflow acceptance evidence beyond this initial deliverable.

## Evidence and implementation prerequisites

Local research and format findings: [RESEARCH.md](compatibility.md),
[verification results](validation.md), and the bounded
classic/new move sample results under `analysis/`.

Reference decoder: [cbh2pgn](https://github.com/asdfjkl/cbh2pgn), MIT license.
Desktop prerequisites: [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

Node and Xcode are present locally; Rust installation and desktop dependencies
are not yet established. Check exact dependency versions, licenses and platform
requirements when preparing the implementation plan. This workspace has no Git
repository; specification commit and isolated product repository setup follow
the required design/plan review gates. No application scaffold or dependency
installation is included in this design-writing stage.
