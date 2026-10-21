# Local playing-style report

**Style report** (`STYLE_REPORT`) is available from Home → Lists in the database
window and Home → Database in a game window, and from command search. Choose a
player in Players, or use the White player of the displayed game. The report
uses the open database independently of the current game-list search.

## OpenFile's independent measures

These are descriptive observations from recorded standard-chess main lines,
not personality ratings, measures of playing strength or replicas of the
reference application's grades. No player name enters a metric calculation.
No engine, stored evaluation, online account or private service is needed.
Engine analysis cost is **zero**. Intent, soundness, positional understanding
and tenacity cannot reliably be determined by the material/move counts below.

Every card shows the numerator, denominator, percentage and separate White and
Black tallies. A zero denominator displays **— / Insufficient evidence**; a
measured zero displays **0.0%**. Fewer than 100 observed moves for a move rate,
or 20 eligible games for a game rate, is labelled **Small sample**. These are
OpenFile's disclosure thresholds, not statistical significance or native
thresholds. Each colour is flagged separately. There is no comparison
population, percentile, normalization or universal aggressive/positional label.

| Measure | Numerator | Denominator |
|---|---|---|
| Checking moves | Player moves giving check, including mate | All observed player moves |
| Capturing moves | Player captures, including en passant | All observed player moves |
| Early queen move | Games with a queen move in the first 10 own moves | Games reaching 10 own moves or showing that event earlier |
| Kingside / queenside castling by move 20 | Games castling on that side within the first 20 own moves | Games reaching 20 own moves or already castled within that window |
| Opposite-side castling | Games in which both kings castle on different sides | Games with both castles observed |
| Queenless by move 30 | Games with a position containing neither queen by ply 60 | Games reaching ply 60 or showing that event earlier |
| Material imbalance at move 20 | Games with different White/Black inventories of knights, bishops, rooks or queens at ply 40 | Games reaching ply 40 |
| Sustained material deficit | Games at least 3 material points down after an own move and still so before the next own move | Complete observed lines, or lines where the event occurs before the replay cap |
| Endgame reached | Games reaching a position where each side has at most 13 non-pawn material points | Complete observed lines, or lines where the event occurs before the replay cap |

Material weights are P=1, N=B=3, R=5, Q=9; kings have no material value.
Promoted pieces count by their resulting type. Queenlessness means an observed
position without queens; later promotion does not erase that earlier event.
A capture is not necessarily an exchange; a sustained deficit is not necessarily
a sacrifice. Short games and incomplete observation windows are excluded from
the corresponding denominator instead of being counted as negative evidence.
This event-or-full-window rule itself favours observed early events in short
records, so rates should be read as the stated sample fractions.

## Selection, limits and supporting evidence

The primary player is an exact database player ID. **Include another spelling /
identity** searches the database's own name list and lets the user explicitly
add up to 15 more identities. Names are never merged automatically. An entry
matching multiple chosen identities is counted once; if both sides match,
White is used unless Black-only was requested. This unusual case may indicate
that two distinct people were selected as aliases.

Colour, inclusive year range and minimum opponent Elo are applied **before**
sampling. Unknown years/ratings are included without a corresponding filter
and excluded when that filter needs the missing value. Unknown results are
allowed because the board measures do not use results. Deleted games are
excluded and counted. Text entries are excluded. Metadata-unknown counts refer
to all nondeleted identity matches before the optional filters, rather than
only the sampled games.

A single header scan retains the latest dated games, then highest entry number
for date ties. Undated games follow dated games. The default is 500 selected
games, configurable from 1 to 2,000. This is a recent sample, not a random or
career-representative sample. The sample is chosen before move decoding;
skipped games are counted and are not silently replaced with older ones.

The report identifies the database, entry count, exact selected identities,
criteria, eligible/selected/analysed counts, actual analysed date range and
colour split. It separately counts skipped setups, unreadable, unsupported
and partial move data. Setups and Chess960 are excluded to keep the opening
windows comparable. Stored annotations and evaluations are not used. Only the
main line is measured, up to 400 plies per game. A cut line contributes its
observed move rates and completed fixed windows; an unobserved later endgame
or deficit is not turned into a measured absence.

**Supporting positions** opens up to eight examples per metric, at most one
per game, at the original node in the game viewer. **Analysed games** lists all
analysed entry numbers and permits inspection of negative examples too. Evidence
comes from computed positions rather than reputation or annotation labels.

## Responsiveness and source validity

The scan/replay runs in a blocking worker outside the UI thread, using bounded
header chunks and an O(sample limit) latest-game heap. Decoding is sequential
and uses the reader's existing per-record limits. Replay checks cancellation
between plies. Progress distinguishes the full-database scan from sampled
replay. Stop and Close discard pending results. Each run has a unique identity;
older progress/results cannot replace a newer run, even for the same player.

The registry cancels superseded jobs and cancels/removes them on database close
or edits. Publication verifies source identities, lengths and modification
times. Supporting-position navigation revalidates the published job and source
signatures. A changed source requires a fresh report. This follows the existing
reader's filesystem-signature model; it does not hash an entire commercial
corpus. The only retained backend state is a current job watermark and source
signatures. There is **no result cache**, so changes to criteria or the
measurement rules always recompute. No report/cache is written beside a source
database.

## Reference investigation

### Scope and identity

Target `extracted/update/CBase18.exe` in the separate research workspace, x86-64 PE, SHA-256 `8d633b10ad97bd9f8fbbc33a89c279397850ca759c8375093deda4a9485e8dee`. Fresh, independently owned REA stdio session `style-20261020-b`; Ghidra 12.1.4 default auto-analysis completed successfully in 395 seconds. REA/Ghidra produced fresh decompilation and assembly results. The Windows target was not executed. Observations below concern shipped machine code and resources, not observed GUI execution. Raw research outputs are retained separately in `analysis/playing-style-20261021/` in the research workspace; unrelated sessions and artifacts were preserved.

Prior required evidence was read before opening: `scripts/rea-reports-session.mjs`, `analysis/native/style-report-command.json`, `style-report-execute.json`, `analysis/ui-native/style-pc.json`, `res-style.json`. The last two cover UI color themes at 0x14093ba40, commands 0xa01c..0xa021; they do **not** establish report computation. Raw RTTI is supporting structural evidence only; decompilation corrected the initial inference that formatter vtable slot 0 was a destructor: it is the CSS renderer 0x1404af820.

All named function addresses below are preferred VAs in this exact executable. `pseudocode/*.c` is extracted decompiler text; original tool results retain full Evidence records. `dimension-theme-weights.json` and `theme-labels.json` are derived tables, with extraction script retained.

### Command, selection, and options — direct code observations

- Shipped command declaration `M_KC_STYLE_REPORT = 653`; keyboard map reports `P`. App message `M_STYLE_REPORT=51547 (0xc95b)` is the report title, `M_TX_STYLE_REPORT=51548` the tooltip. These are declared application/message identifiers, not asserted Win32 menu IDs.
- Prior dispatch 0x1408b4830 checks global readiness 0x140940850, active DBListInterface, casts to CBaseInGridDBListWin and calls virtual +0x4b8. Execute 0x1408b6cb0 calls +0x4b0. CBaseDBGridPlayerListWin main vtable 0x1411cb1e8 overrides these with 0x14073a6f0 (enable, delegates DBListInterface +0x48) and 0x14073c540 (execute). Looking only at the parent CBaseInGridDBListWin misses derived overrides.
- 0x1402bf540 returns a player only when selected-item count is exactly one. 0x14073c540 then copies that player name and calls 0x1407b5d90. If not exactly one, 0x1402bf5b0 resolves the selected player rows into a vector and 0x1407b1840 iterates them, creating separate reports.
- 0x1407b5d90 opens shared `GetPlayerDossierTypeVariables` options through 0x1406bf5c0. Dynamically generated variables are `V_WITH_BLITZ`, `V_WITH_RAPID`, `V_WITH_LONG`, `V_WITH_SIMULS`, `V_YEARS`. 0x1406bed90 defaults Long=true; Simuls/Blitz/Rapid=false. Years use a RangeVariable bounded 0..9999; 0x140481920 supplies an initial range from player-encyclopedia data when available. Birth-plus-16/current-or-death is a plausible interpretation of those fields, not yet fully identified.
- Accepted options pack Blitz=1, Rapid=2, Long=4, Simuls=8; year bounds are copied separately. No dedicated fixed resource dialog/control-ID template was established: this path constructs variables dynamically. Report choice UI is conditionally constructible, but this style entry passes the flag that omits it.
- 0x1407b2220 looks up the name in the Cbg database player index via virtual +0x2c0 with mode 2. Cbg implementation 0x140132350 resolves player index ID then retrieves its game array. Mode 2 returns games from both colors; modes 0/1 do further per-header color filtering. No intersection with the active game-list filter appears in this player-report path. Alias handling inside the lower name-index lookup remains unresolved; no explicit alias expansion was observed here.
- Separate game-selection report handler 0x1408d4e60 maps the selected game-list rows to actual game numbers, calls the same style generator with an empty player name and flags 0xf. This is the average-over-selected-games route. Its code rejects a database failing the Cbg cast and shows `Style Report only on CBH and 2CBH databases` (0x1412110b8). The actual command mapping for this additional handler has not been fully traced.
- Style mode has no report-specific N-max cap in the traced options/dispatch. 0x1407b1840's `N Max Games` input (default100, range1..1000) is gated on mode2, the blunder-report path, not this style mode1. This does not prove unlimited file format capacity.

### Candidate games and denominator populations

0x1404a3e30 scans candidate headers, detects whether one player is common to every header (0x140492710), filters headers, and loads full games. The common-player detector counts canonical displayed names obtained with selector99; it returns an empty name when none occurs in every game. Mixed player sets become an average report, rather than fabricating a single player's side.

Header predicate 0x140481a20:

1. Rejects signed first header byte <0 (deletion flag in the format) and stored move-count helper <5. Helper 0x1401cea40 returns header+0x8a only when header version marker+2 is 0x101; otherwise0. The exact stored count's fullmove/ply convention should be reconciled with the format decoder before reproducing it.
2. Applies inclusive nonzero lower/upper years, extracting year by >>9 from the packed date. Zero means no respective bound.
3. Blitz if event type bits &0xe0 equal0x20, or event name contains `blitz` or `bullet`, case-insensitive. Rapid if bits equal0x40 or name contains `rapid`. Long is neither Blitz nor Rapid.
4. Simul if low event-type bits &0x1f equal7 or event name contains ` sim`. Simul takes precedence, then Blitz, Rapid, Long; the selected category bit decides inclusion.

Theme eligibility 0x1404a48a0 differs from raw games/results denominators: at least4 analyzed position records; exclude draws with stored game length <26; require the player's appropriate color (role0 White,1 Black,2 average permits both). Sacrifice theme3 adds a favorable-or-drawn estimated-result condition. Thus “theme games / games” is not always “theme occurrences / all selected games.”

0x14048c8a0 computes each theme/color record. It counts eligible games and games whose PropertyValue exceeds that property's threshold; accumulates property values, squares, actual game points, and a rating subset. Positive-occurrence means/scores use occurrence count; frequency uses occurrences/eligible. When no positive occurrences exist, it clears the entire record, including stored denominator. Rating subset requires both ratings nonzero and clamps opponent rating to within400 of player rating before expected-score computation. Games with missing ratings can still contribute thematic frequency and actual result score.

Generic aggregate helpers: 0x1404988b0 averages a double callback over every processed Data record and returns0 if empty; 0x140498ac0 returns predicate count / total Data records, or0 if empty; 0x140498c00 sums a callback; 0x1404a1f20 returns count, actual points (1/.5/0 from player's side), plus rating-derived performance metadata. These have different denominators and should not be conflated.

### Actual native dimensions and calculations

0x1404acb40 emits IDs0..7. 0x1404ad8d0 resolves their labels:

| ID | Label | Raw score | Grade wrapper |
|---|---|---|---|
|0|Score|0x1404abbd0|0x1404a86e0 case0|
|1|Theory|0x1404ad090|0x1404acf60|
|2|Decided Games|0x1404a76e0|0x1404a75b0|
|3|Tenacity|0x1404ace00|0x1404accd0|
|4|Aggressiveness|0x1404a6cc0|0x1404a6ba0|
|5|Risk|0x1404ab970|0x1404ab850|
|6|Positional Play|0x1404aa050|0x1404a9f30|
|7|Endgame Affinity|0x1404a7870|0x1404a7750|

These are composite scores relative to an embedded reference profile. 0x14049a130 initializes a large hard-coded profile at global0x142264970 and copies it; 0x1404a6530 puts that reference after the player profile at+0x28e0. HTML explicitly labels reference frequencies `GMs`. This is evidence of intended GM comparison, but **the sampled players, database, dates, sample size, selection criteria and normalization provenance of that embedded reference were not recovered**. It is not recomputed from the user's current database and is not a runtime percentile cohort.

Let `D(x)=playerScalar(x)/referenceScalar(x)-1`, with reference at player base+0x28e0; traced raw routines generally contribute0 when the reference denominator is0.

- **Score** sums `1.2*(White points/game / reference White points/game -1)` and `1.0*(Black points/game / reference Black points/game -1)`, selecting alternate count-positive stats when available and otherwise fallback actual-score stats; the alternate subset provenance remains unresolved. It is not simply win rate.
- **Decided Games** is `-1.5*D(0x258) - D(0x260)`. Aggregator/callbacks establish those fields as White and Black draw fractions respectively. It rewards fewer draws relative to reference.
- **Tenacity** includes `-.6*D(0x258) -.5*D(0x260) -D(0x240)` (the latter short-draw fraction), plus `3*D(0x288)` (mean draw length) when there is any draw, `+.2*D(0x338)` only above the reference, and a conditional `-.4*D(0x328)` term. 0x328/0x338 derive per-move transition counters averaged over analyzed moves; exact semantic labels remain unverified. Native short-draw predicate0x1404a5bc0 is draw result and stored length<26.
- **Theory** uses signed relative ratios of two repertoire aggregates (+0x2c8 White weight1, +0x2e0 Black weight1.5), support-map statistics (weights.2,.15), and engine evaluation averages at+0x2848 and+0x2850 (weights+2 and−2.5). Exact repertoire encoding and support-map statistic semantics remain partially unresolved.
- **Aggressiveness** and **Risk** use theme-weight tables below plus scalar terms. Aggressiveness additional scalar terms are `-.1*D(0x340) -.3*D(0x348) -.2*D(0x310) +.2*D(0x370)`. Risk additional terms are `-1.9*D(0x340) +.5*D(0x308)`. +0x308 is fraction with a nonnegative material-configuration marker; +0x340/+0x348 derive per-color position-property coverage counts (exact semantics unresolved). Do not label them “simplification” solely from nearby observations.
- **Positional Play** uses the full 32-term table in `dimension-theme-weights.json`, plus `.6*D(0x340)`. It includes queenside play3, center play2.5, White space advantage2, Black space advantage1, pawn weaknesses, strong squares, bishop pair, piece activity, fianchetto color asymmetry, and negative sharpness/attack terms.
- **Endgame Affinity** uses endgame frequency (+positive relative deviation×4), performance differences with sample guards, and equal/better/worse/technical endgame point-rate comparisons. The dimension dispatch requires `count(0x1a8)+2*(count(0x208)+count(0x1d8)+count(0x178))+count(0x148)+count(0x118)>27`, else grade Undetermined. Individual branches also require counts>10, >20, or >50 and performance-difference bounds. The full 295-line routine is retained; some complex fallbacks and repeated contributions need assembly validation before reproducing.

#### Theme weights and comparison rule

`theme-labels.json` maps all41 property IDs by actual native label-switch0x140497970 and shipped message resources. `dimension-theme-weights.json` contains complete Aggressiveness, Risk, Positional Play term records; coefficients, color selection and positivity flags were decoded from the raw score constructors.

0x1404a6870's **assembly** (retained `theme-combine.asm`) establishes the rule: loop White and Black; include each weight whose color is that side or Both. Extract player/reference frequency from the record's preferred nonzero precomputed frequency, then alternate positive frequency, then occurrence/eligible fallback. Skip unavailable reference (no sample/frequency). Contribution=`weight*(playerFrequency/referenceFrequency−1)`. A term marked positive-only is omitted unless this weighted contribution is positive. Both-colors means separately summing two contributions, not averaging their frequencies first.

Aggressiveness positive-only terms for Both: King's Attack2.9, Opposite Castling1, Sharp1.2, Sharp Opening.5, Calm−.2, Sacrifice1, Gambit1; Fianchetto White−.3 signed, Black+.4 positive-only.

Risk positive-only Both: Symmetry−.6, Asymmetry+.5, Opposite Castling+.2, Gambit+.3, Sharp+.9, Sharp Opening+.6, Calm−.4.

The aggregate-theme registry0x1404941a0 includes IDs0..40 then removes15 (Active Play), although the positional formula still contains an Active Play term. Do not silently “repair” this when interpreting compatibility.

#### Grades, display percentage, and missing data

Native grade labels via0x1404ad240:0 Undetermined;1 Very Low;2 Low;3 Medium;4 High;5 Very High. Grade wrappers normalize the raw score against per-dimension plotting bounds from0x1404a89a0, multiply100, lower-clamp0, then compare25/60/80. Very Low's <0 branch appears unreachable for finite values after that clamp. Several wrappers first inspect the existing output-struct grade before normalization; its initialization/intent needs further runtime or assembly-context validation. Therefore do not advertise exact grade compatibility by implementing just these thresholds.

Bounds decoded from0x1404a89a0: Score[−1.5,1], Theory[−2.5,3], Decided[−.5,1], Tenacity[−2,2], Aggressiveness[0,6], Risk[0,2], Endgame[0,3]. Positional[0,7], confirmed from case6 at0x1404a8d57 and its SSE arguments at0x1404a8d9e in `grade-ranges.asm` (the decompiler loses those arguments). Four additional per-dimension cutpoint values are constructed but grade wrappers above use hard-coded25/60/80; do not assume those cutpoint vectors define the displayed grades.

0x1404ae240 renders raw normalized percentages in a table; grade0 displays `Not enough games`. 0x1404a88d0 computes graph width as truncation of99×clamp(normalizedFraction,.01,1), so graph width is distinct from displayed percent and maxes99. No universal “minimum N games” was established for all dimensions; available sample guards vary by metric.

### Analysis dependencies, progress, cancellation, navigation and files

- 0x140493480 splits games into workers0x140492ef0. Worker count derives from configured processor count minus2 with minimum2; when <400 games it caps worker count at4. It creates an analysis context via0x140c3c790 per worker. This allocates an **in-process** context through0x140001050 (a pool/initializer in the same executable), not an observed external UCI process.
- Per game0x14048f1b0 calls header extraction0x14048efb0, detailed position analysis0x14048e260, and result/endgame analysis0x14048f430 when board status permits. MaterialDescriptor creation0x14048f2b0 identifies phase boundaries. Some cached per-position eval records are reused; missing records trigger0x140495d70/0x140495fa0 →0x140c3dda0 →0x140c3db90 →internal0x140001480. Evaluation scores are transformed by side and averages scaled. This establishes an internal evaluation dependency for the native report; no cloud/service request or external-engine executable launch was found in this traced path. Exact engine identity, search strength, and complete transitive behavior remain unresolved, and static absence is not runtime proof.
- Result classification0x14048f430 samples a phase-relevant position, treats abs(eval)<50 as equal, abs(eval)>=71 as favorable/unfavorable subject to side, and falls back to actual result if classification remains unknown. These are raw evaluation units, likely centipawn-scale (the aggregator scales by.01), not a verified public engine API contract.
- Header scan0x1404a4300 creates progress above5000 candidates, updates/checks cancellation every2500. Full-game loading0x1404a4040 creates progress above1000 and updates/checks every2500. Analysis creates progress above200, reports worker counts, pumps UI while waiting with500ms sleeps. Cancellation sets shared+0x68, workers stop, incomplete default result records are removed, and output cancel byte+0x25 is set. The top generator still has rendering after that processing but returns false for cancelled output; exact partial-report UI behavior was not executed.
- 0x14048faa0 builds frequent White/Black opening variations. Its occurrence/depth-like parameter adapts to game counts:9 below101;7 otherwise;6 above300;5 above500;4 above800;3 above1000;2 above2000. Exact parameter meaning requires checking its callee.
- Example selector0x140496300 collects per-theme and color positive examples, sorting/trimming to100 candidates; formatter0x1404adc20 displays at most7 per theme/color. 0x140481da0 builds a game-link representation, 0x140c67930 emits an actual `<a href>` using virtual link target/text functions. Target protocol/activation semantics remain unresolved.
- Formatter0x1404af440 emits HTML sections `hero`, `charts`, `statsTables`, `games`, including player/year/rating summary, eight grades, observations, frequent opening tables and theme example games. Native document path0x140712960/0x140712890 wraps equivalent formatted content in StyleReportText and opens it through application document manager.
- 0x1404b0190 creates user Reports/StyleHTMLs and writes `{player}-Style.html`, or `Average-Style.html` for an empty name. It kills the existing path before writing; this is regenerated output, not evidence of a cache hit. 0x1404aeab0 can open the resulting existing file. Per-position evaluation reuse was established in memory, but persistent style-analysis cache format, invalidation, and reuse across runs were not established. A `Write CSV Data...` progress string occurs but a CSV output side effect was not proven; do not infer one from that string.

### Direct observations versus inferences and remaining limits

Direct: command/virtual dispatch, selected-player versus multi-player iteration, name-index both-color query, category/year/header filtering, theme eligibility and denominator code, eight labels, actual weight constants/arithmetic, hard-coded reference profile, internal evaluator calls, progress/cancel state, HTML/native-document creation, example count caps.

Inferred with bounded support: deleted header flag interpretation is consistent with existing format decoding; reference is intended as GM population because native output explicitly calls it GMs; scalar evaluation units likely centipawns due .01 scaling; default year range likely player birth+16 through current/death. These require reconciliation with format/runtime evidence where exact compatibility matters.

Unresolved: alias expansion in lower name lookup; exact current-list-filter interaction below the returned player index; reference population/cohort provenance and sample sizes; several scalar property names and all detailed property thresholds; full phase-boundary conventions; full evaluator identity/settings; grade output-struct initialization; runtime cancellation presentation; concrete dynamic control IDs/actual Win32 command IDs; example-link protocol and activation; persistent analysis cache and invalidation. No compatibility, parity, or native-score equivalence claim is justified by these partial static results.

### Evidence index

- `entry-functions.json` ev_7de5e28f8b308358da06f4d1b567ea8bce80e181387a4c2e6483c325f3c8feba
- `report-entry-helpers.json` ev_c57acbbf8851790eb8a9ff576f6a08ef319afbf2a40bf750ebc5e42805a6720a
- `report-generator.json` ev_a75da433de8db9f01534423007e9b6cabc98aa6db6bb64f061319ff969c48948
- `style-top-generator.json` ev_7ae6e2c45d50db5045cebfb88c8bb2021ce5b148c69fdae84f96235f5aa9c56a
- `style-data-and-render.json` ev_804b3ad225bf1e32b5c131d4b83ea4765c89cb57d7f09e399044918de69d6e75
- `style-core-pass2.json` ev_7d884cb2f912eff8bb5d911d07de6d21d1d70d3ac0443d33f54aea4e7d23db55
- `style-math-and-worker.json` ev_04439ed1beda85a81d4947f4c74ff5f42480746018a23a0807ace9e71f1945c4
- `style-grades-and-analysis.json` ev_fae7ae174ce77ca6cca7d9083f79ae5b0aa75998c6394bbe2d3f2118e76a37f1
- `style-dimension-math.json` ev_df287e8e3ebc3c3f06ba06b1a0631ca13042020063f36a415557c282a54d1c51
- `style-raw-scores-and-engine.json` ev_b4e4a1a37dc5184e9c898fd10f55a8546ba548a22391d7eb3293d633d75fe8e1
- `style-theme-combine.json` ev_d3127f852c10cda245aacfe6b78a368ec19906fc0b861a84ace9dab26b78d981; compare `theme-combine.asm` because decompiler discards its SSE-only arithmetic
- `style-observations.json` ev_787418e624ff4728a1b7b0a0bd0c903bc3fe244233a0ca15006d84078ce6de8c
- `style-engine-identify.json` ev_6b86896c599b19cca0dc82fd97a97bac82a041e439496d73b333688b71a94745
- `style-themes-labels-eval.json` ev_8a7fa0845db44fba58d5dbcb742e9669b94da640a1a934228b1766a0c4c3d67f
- `style-theme-names-population.json` ev_3e73aaf7f98d90d6850efd7e889772320f4ae322ea355e533cb740b2cd1cc6af
- `style-eligibility-and-filter-selection.json` ev_241d89624fcc36586125971d99abba6299641cd3c3545fd4544fddb05612671f
- `style-player-game-query.json` ev_8779dcf094da4bd7cc9c96efcd0a4c7809a1a6b5ca24a18c3c85d620d2170581

Caution: Ghidra sometimes loses live register parameters or SSE returns under inferred prototypes. For example0x1404a3e30's decompiler shows a spurious0 first argument to0x1404a4300; actual assembly leaves RCX holding the collection. Another example is0x1404a6870, whose decompiler looks like an empty loop but whose assembly clearly performs frequency-ratio arithmetic. Treat retained machine instructions as primary when these disagree.

Cleanup: final evidence bundle and analysis snapshot were saved successfully. REA close returned `cleanup_incomplete` for provider-client; a subsequent OS process inspection confirmed both this session’s Ghidra Java process and its REA Node client were gone. The separate failed-session A idle Node client was explicitly stopped after exact command-path/PID verification. No unrelated session was stopped. Unexecuted post-close retry requests remain as provenance only.


## Real-player acceptance and performance

Acceptance uses the available **Mega Database 2025** classic database,
11,151,119 entries (512,951,520-byte CBH index). Exact identities are
**Carlsen, Magnus #65470** and **Nakamura, Hikaru #286135**. Neighbouring name
list entries were inspected; distinct people such as Carlsen, Morten and
Nakamura, Julia Yumi Ono were not merged. No extra aliases were selected.
These examples establish the selected identities in this database, not that
all possible aliases anywhere in the corpus have been found.

Both reports use both colours, no year/rating restriction, unknown metadata
included, deleted games excluded, latest-date sampling with a 500-game limit,
and the 400-ply replay cap. Time controls are not filtered: the samples mix
recorded event types, and **are not long-game-only reports** like the native
default. Filtering named time controls is not implemented because the reader
has no reliable universal time-control field for these records.

| Context | Carlsen | Nakamura |
|---|---:|---:|
| Eligible nondeleted games | 6,925 | 9,288 |
| Selected / analysed | 500 / 500 | 500 / 498 |
| Analysed White / Black | 252 / 248 | 250 / 248 |
| Actual analysed dates | 2024.05.14–2024.10.01 | 2024.06.13–2024.09.24 |
| Skipped / cut at replay cap | 0 / 0 | 2 unreadable / 0 |
| Checking moves | 1,477 / 22,485 (6.6%) | 1,453 / 24,647 (5.9%) |
| Capturing moves | 4,736 / 22,485 (21.1%) | 4,686 / 24,647 (19.0%) |
| Early queen move | 166 / 500 (33.2%) | 234 / 498 (47.0%) |
| Kingside castle by move 20 | 380 / 498 (76.3%) | 345 / 497 (69.4%) |
| Queenside castle by move 20 | 42 / 498 (8.4%) | 55 / 497 (11.1%) |
| Opposite-side castles | 43 / 402 (10.7%) | 36 / 372 (9.7%) |
| Queenless by move 30 | 202 / 440 (45.9%) | 177 / 459 (38.6%) |
| Non-pawn imbalance at move 20 | 257 / 490 (52.4%) | 249 / 491 (50.7%) |
| Sustained material deficit | 83 / 500 (16.6%) | 78 / 498 (15.7%) |
| Endgame reached | 320 / 500 (64.0%) | 310 / 498 (62.2%) |

`src-tauri/examples/playing_style.rs` reads the source without write access,
checks exact identities, reports timing and optionally exports the analysed
samples to a separately chosen PGN path for validation. A separate Python
implementation using python-chess 1.10.0 replayed those PGNs and independently
matched all ten numerators/denominators, both colour splits, and every shown
evidence move's SAN/ply. The private PGNs and report JSON are temporary external
validation artifacts, not product fixtures or redistributed databases.

Release example runs on the development Mac with filesystem cache warm:

| Stage | Carlsen | Nakamura |
|---|---:|---:|
| Prepared name-list lookup | 0.005 s | 0.002 s |
| Full 11.15M-header scan and bounded selection | 0.172 s | 0.131 s |
| Replay, metrics and report assembly | 0.197 s | 0.154 s |
| Total report computation | 0.369 s | 0.285 s |
| Engine processes / engine analysis time | 0 / 0 s | 0 / 0 s |
| Whole example process, including separate PGN export | 0.92 s | 0.41 s |
| Whole-process peak resident memory | 12.4 MiB | 12.3 MiB |

These times exclude opening the database and preparing/finding the name-list
identity. They are representative local warm-cache measurements, not promised
latencies on cloud placeholders or remote disks. Native debug builds take
longer: final 500-game UI runs took 3.04 s for Carlsen and 2.96 s for Nakamura. Selection memory is bounded
by the chosen sample and one 8,192-header chunk; decoded game memory is bounded
by the reader's per-game limits. The existing player-name index has its own
larger storage/build cost, described in [player statistics](player-statistics.md).

Native validation uses byte-identical APFS clones of the same source family,
made read-only in temporary storage, and a separately identified validation app
with separate settings. This avoids macOS blocking the temporary app's access
to Documents, and leaves the user's installed app and reference database
unchanged. Source signatures are checked before and after acceptance.


### Native UI evidence

These are computed reports in the running macOS Tauri app, not preview fixtures.
Open a game or select the exact player in Players, choose **Style report**, keep
both colours and the 500-game limit, then choose **Generate report**. Inspect
**Activity & openings**, scroll to its remaining cards, and switch to
**Structure & endings**. The fixed player heading remains visible while scrolling.
The full analysed-game table is also available, including negative examples.

| Player | Activity | Structure/endings | Supporting position |
|---|---|---|---|
| Carlsen | [Report](media/style-carlsen-activity.png), [remaining activity cards](media/style-carlsen-activity-more.png) | [Report](media/style-carlsen-structure.png) | [Entry 11148542](media/style-carlsen-evidence.png) |
| Nakamura | [Report](media/style-nakamura-activity.png), [remaining activity cards](media/style-nakamura-activity-more.png) | [Report](media/style-nakamura-structure.png) | [Entry 11146243](media/style-nakamura-evidence.png) |

All retained screenshots were visually inspected. Carlsen's **Endgame reached**
support opens Carlsen–Firouzja, entry 11148542, at ply 90 / 45...Qxg7; both
sides then have nine non-pawn material points. Nakamura's **Queenless by move 30**
support opens Nakamura–Naroditsky, entry 11146243, at ply 15 / 8.Kxd1; the board
has neither queen. The native board, highlighted notation and move status were
checked against these moves. Source-bound navigation opens a fresh document
rather than reusing an existing potentially edited game tab.

The original CBH and validation clone have SHA-256
`1750ba956d38339091392c7adea9e26bb2cfce1f1438d876fdeb76d8e0789845`.
All nine original database-family file sizes, modification times and file
identities were unchanged after acceptance. No private PGN, database, native
code or reference artwork is included in this repository.


Native cancellation was additionally exercised during the full-database scan:
**Stop** leaves [Report stopped](media/style-cancelled.png), re-enables criteria,
and publishes no partial report. A WebKit-specific form default initially
restarted the report after Stop synchronously changed that button back to a
submit button. The click default is now prevented; a failing-then-passing
regression checks that contract, and the rebuilt native app confirmed the fix.
A subsequent explicit Generate completed the same 500-game Carlsen sample.
