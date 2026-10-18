# Third-party notices

OpenFile is licensed under GPL-3.0-or-later.

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

Chess piece images are the "cburnett" set by Colin M.L. Burnett, licensed
GPL-2.0-or-later, as distributed in lichess-org/lila (public/piece/cburnett).
They are embedded unmodified in src/board/pieces.ts.

The interface follows ChessBase 18's command vocabulary, ribbon grouping,
default shortcuts and theme colours, recorded through local interoperability
research. No ChessBase icon, bitmap, font, text resource or code is included;
icons come from Lucide and are tinted with recreated theme colours.

Classic CBG token tables and algorithm details are adapted from cbh2pgn,
Copyright (c) 2022 Dominik Klein, MIT license. Reference revision
42b3592738062db1f768239e85df1b98cb1cead9. Full notice: licenses/cbh2pgn-MIT.txt.
OpenFile adds legal validation, bounded reads and corrected capture state.

The inventory covers all 429 locked external Rust packages, including platform
and build dependencies, and nine production npm packages (including @types/react and csstype, which
lucide-react declares as a peer dependency; they contain types only). These are dependency
notices, not copied implementation sources. Some crate archives omit their
license files; those notices were retrieved from the exact recorded upstream
Git revisions. Shared winapi-rs and libappindicator project notices are marked
explicitly; the npm dialog SPDX copyright accompanies shared Tauri license
texts. The recorded origin links and SHA-256 hashes preserve provenance.

Build-only npm tools are resolved in package-lock.json and are not shipped in
the runtime. A local UCI engine is separately selected by the user and is not
part of OpenFile's distribution. Native system libraries and WebView
runtimes are platform prerequisites. Source distributions must include this
file, LICENSE and licenses/. The macOS bundle includes these resources; CI
executable artifacts include them alongside the binary.

The ECO codes and opening names in crates/chess-core/data/eco/ (a.tsv to
e.tsv) are the lichess-org/chess-openings data, dedicated to the public domain
under CC0 1.0 Universal, revision a6189a30dc273ccb21fc2536a9a2fefd5592a67a
(2026-10-06), kept unmodified. Full text: licenses/chess-openings-CC0.txt.
