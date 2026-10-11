# Third-party notices

Chess Workbench is licensed under GPL-3.0-or-later.

The Rust chess rules and notation implementation uses shakmaty by Niklas Fiekas,
licensed under GPL-3.0-or-later. Dependency versions are recorded in Cargo.lock.
Serde, serde_json, thiserror and tempfile use MIT OR Apache-2.0; same-file uses
Unlicense OR MIT. Tauri and its dialog plugin use MIT OR Apache-2.0. React,
React DOM and scheduler use MIT. Resolved versions, complete declared license
expressions and checksummed notice files are recorded in
[licenses/dependencies.json](licenses/dependencies.json).

No commercial database, proprietary executable, image or extracted resource is
part of this source repository.

PGN parsing uses pgn-reader by Niklas Fiekas, GPL-3.0-or-later.

Classic CBG token tables and algorithm details are adapted from cbh2pgn,
Copyright (c) 2022 Dominik Klein, MIT license. Reference revision
42b3592738062db1f768239e85df1b98cb1cead9. Full notice: licenses/cbh2pgn-MIT.txt.
Chess Workbench adds legal validation, bounded reads and corrected capture state.

The inventory covers all 429 locked external Rust packages, including platform
and build dependencies, and five production npm packages. These are dependency
notices, not copied implementation sources. Some crate archives omit their
license files; those notices were retrieved from the exact recorded upstream
Git revisions. Shared winapi-rs and libappindicator project notices are marked
explicitly; the npm dialog SPDX copyright accompanies shared Tauri license
texts. The recorded origin links and SHA-256 hashes preserve provenance.

Build-only npm tools are resolved in package-lock.json and are not shipped in
the runtime. A local UCI engine is separately selected by the user and is not
part of Chess Workbench's distribution. Native system libraries and WebView
runtimes are platform prerequisites. Source distributions must include this
file, LICENSE and licenses/. The macOS bundle includes these resources; CI
executable artifacts include them alongside the binary.
