# Local playing style

Implement STYLE_REPORT from the selected database player (or shown game's player), using the existing Rust/Tauri/React app. The user requests independent decisions and continuous execution through native UI evidence, local checks, focused review and an unmerged passing PR.

The reference research is a separate REA/Ghidra investigation. Its findings and uncertainty belong in docs/playing-style.md. No proprietary implementation, assets, distributions or prose is copied. Reference databases remain read-only.

OpenFile will measure recorded board behaviour: checks and captures per player move, early queen development, castling choices, exchanges/queen retention, material imbalance and endgame occurrence. These relate to the reference's resource themes without pretending to reproduce hidden personality scores. No engine is necessary for these measures; ability, intent and sound sacrifices cannot be established from material alone.

Use exact selected player IDs and explicitly chosen additional identities as aliases. Filter colours, years and opponent rating before taking a bounded deterministic latest sample. Count exclusions and unknown metadata honestly. Traverse only validated standard-start main lines; exclude setups, unsupported/corrupt/partial moves and count those exclusions. Bound both sample size and replay length; expose truncated games and all analysed entry numbers. Metrics include numerator, denominator, insufficient-evidence status, per-colour values and bounded clickable supporting positions.

Jobs run off the UI thread, show scan/replay progress and stop cooperatively. A unique job ID prevents late progress/results replacing newer runs. Source fingerprints are checked before publication. No persistent result cache: recomputation avoids stale cache semantics and keeps reports separate from sources.

Native verification uses actual Carlsen and Nakamura data, independently checks representative results, exercises game/node navigation, and saves visually inspected screenshots in docs/media. Synthetic evidence stays in tests. Document performance, sample bias and unsupported calculations.
