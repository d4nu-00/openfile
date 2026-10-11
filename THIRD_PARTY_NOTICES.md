# Third-party notices

Chess Workbench is licensed under GPL-3.0-or-later.

The Rust chess rules and notation implementation uses shakmaty by Niklas Fiekas,
licensed under GPL-3.0-or-later. Dependency versions are recorded in Cargo.lock.
Serde is MIT OR Apache-2.0; thiserror is MIT OR Apache-2.0. Full dependency and
distribution notices are audited before source publication.

No commercial database, proprietary executable, image or extracted resource is
part of this source repository.

PGN parsing uses pgn-reader by Niklas Fiekas, GPL-3.0-or-later.

Classic CBG token tables and algorithm details are adapted from cbh2pgn,
Copyright (c) 2022 Dominik Klein, MIT license. Reference revision
42b3592738062db1f768239e85df1b98cb1cead9. Full notice: licenses/cbh2pgn-MIT.txt.
Chess Workbench adds legal validation, bounded reads and corrected capture state.
