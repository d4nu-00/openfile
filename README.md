# Chess Workbench

An independent GPL-3.0-or-later desktop chess database workbench, built with
Rust, React and Tauri. It reads supported classic `.cbh` families, browses games
and variations, edits a working copy, saves PGN and analyses positions with a
local UCI engine. This is the first workbench stage; full ChessBase feature
equivalence remains future work.

The macOS Apple Silicon app has been built and exercised natively. Windows and
Linux native verification is outstanding. The committed three-platform CI
workflow has not been run on remote runners. See [validation](docs/validation.md)
for evidence and [compatibility](docs/compatibility.md) for exact format limits.

## Build and run

Install the [native Tauri prerequisites](https://v2.tauri.app/start/prerequisites/),
Rust/rustup, and Node 24 LTS. The repository pins Rust 1.99.0 and Node's major in
`.nvmrc`; Cargo.lock and package-lock.json pin resolved dependencies.

```sh
npm ci
npm run tauri -- dev
```

For a native executable or macOS app bundle:

```sh
npm run tauri -- build --no-bundle -- --locked
npm run tauri -- build --bundles app -- --locked  # macOS only
```

The executable is `target/release/chess-workbench` (`.exe` on Windows), under
the workspace target directory. macOS bundles appear at
`target/release/bundle/macos/Chess Workbench.app`. Local bundles are unsigned
development builds. Windows needs WebView2; Linux needs its GTK/WebKit runtime.
Signing, notarization and packaged Windows/Linux installers remain outstanding.

## Use

Choose **Open database** and select the `.cbh` file. Keep the matching `.cbg` beside
it for moves, and `.cbp`/`.cbt` for player/tournament names, using the same stem.
Missing optional companions produce diagnostics. Sources stay read-only.
Cloud placeholders may take time to download during the first open.

Select a game, click notation or a variation, and use Previous/Next or the
Left/Right arrow keys to replay. Flip Board changes the viewing side. Keyboard
navigation operates when focus is outside editing fields.

Click a piece and a destination to append a legal move, choose promotion when
required, and use Apply to store edited comments/NAGs. Save PGN writes a separate
PGN file; it does not write CBH. Open PGN imports a bounded collection (up to
2,000 games). Replacement, game close and normal application exit offer
Save/Discard/Cancel for unsaved edits. On macOS Command-Q uses that same guard.

Choose Engine selects an installed UCI executable. Configure its advertised
options while stopped, then Start Analysis. Analysis follows the selected
position and shows legal SAN principal variations, depth, nodes and scores
from White's perspective. Stop/Close Engine stop its process; no engine is
downloaded or bundled. Standard chess is required.

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

The source audit checks Git-tracked files against an explicit allowlist, rejects
private/proprietary file families and home paths, and verifies locked dependency
inventory coverage and notice hashes. Stage intended new product files before
auditing them. Generated builds, private validation reports, external engines,
commercial games and extracted application resources are excluded from source.

After dependency changes, regenerate the notice inventory from Cargo metadata
and installed npm production packages, review it, then rerun the audit:

```sh
cargo metadata --format-version 1 --locked > /tmp/workbench-metadata.json
node scripts/generate-notices.mjs --metadata /tmp/workbench-metadata.json --cache /tmp/workbench-license-cache --fetch
```

The optional private corpus comparator requires python-chess, a separately
obtained pinned MIT reference, and explicit external database/observation/report
paths. Proprietary test games and local research are never product fixtures.

See [LICENSE](LICENSE), [third-party notices](THIRD_PARTY_NOTICES.md) and
[the dependency inventory](licenses/dependencies.json). Bundles include these
notices. Distributing executables under GPL also requires offering corresponding
source; this local work has not published a repository or release.
