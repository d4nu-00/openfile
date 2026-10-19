# Repertoire Workspace design

OpenFile needs a personal White and Black opening workspace that survives restarts and follows positions across databases and move orders. It is separate from generated repertoire databases and game annotations.

The game window gains a Repertoire tab beside Book and Reference. The selected board position shows saved moves in SAN, preferred choice and short notes. A move can be added from the current opening tree or from any legal board move. On the repertoire side, saved moves are candidate answers; on the opponent side, they are expected replies. A compact list of saved positions shows their recorded route and opens each position on a fresh board. The route is for navigation only; positions are identified by their legal chess state (board, turn, castling rights and legal en passant), with move counters omitted. White and Black remain independent even at the same position.

The workspace files live in the app data folder, with atomic replacement and serialized edits. Saved moves are validated when loaded and used; stale or invalid records are shown as invalid and can be removed, never played silently. A separate practice file records attempts. Practice starts on a saved own-turn position, hides the saved choices, accepts a legal move on the board, and grades any saved choice as correct. A different legal move needs another attempt; a preferred choice is named in feedback but alternatives still pass. A later correct answer clears the retry marker.

Coverage uses the current database's Book rows. On the player's turn it lists unsaved reference moves; on the opponent's turn it lists reference replies whose resulting position has no saved legal answer. Each has its game count. Tree absence, limited line following and search-result sampling are labelled. Repertoire edits and practice do not write the source database. Slow tree lookups continue through the existing cancellable position pane flow.

This release is a local, manual workspace: no cloud sync, engine grading, import/export, full tree replay, or automatic repertoire generation. The existing Generate Repertoire command remains separate.
