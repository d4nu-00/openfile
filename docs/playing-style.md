# Playing-style report: recovered native dimensions

**Style report** (`STYLE_REPORT`) is available from Home → Lists in the database
window, Home → Database in a game window, and command search. Select an exact
player in Players, or use the White player of the displayed game. The current
report replaces the initial ten descriptive measures with ChessBase 18's eight
native dimensions, in their original order. It is a **partial reconstruction**,
not a claim of matching a displayed ChessBase report.

| Dimension | Current calculation |
|---|---|
| Score | Recovered raw formula, comparison values and normalization |
| Theory | Unavailable: needs native opening-position taxonomy and evaluation inputs |
| Decided Games | Recovered raw formula, comparison values and normalization |
| Tenacity | Unavailable: exchange-opportunity counters and phase conventions remain unresolved |
| Aggressiveness | Unavailable: essential native thematic and king-safety properties are missing |
| Risk | Unavailable: native thematic, king-safety and asymmetric-material detectors are missing |
| Positional Play | Unavailable: native positional-theme properties and their eligibility are missing |
| Endgame Affinity | Unavailable: native stable-phase, evaluation and performance subsets are missing |

The unavailable dimensions do not use the former capture/check/material measures
under new names. Their cards state the missing dependencies. No engine or private
service is launched for the currently computed result dimensions. A generic UCI
centipawn score would not reproduce the missing native positional properties.

## Verified result calculations

For each player colour, let N be **all processed games** of that colour. A win
contributes one point, a draw half a point, and every other result zero points.
Unknown results remain in N, contribute zero points and zero draws, and are
explicitly disclosed. Forfeits use their canonical win/draw/loss kind. This is
the recovered native aggregation rule; unknown is not silently interpreted as a
known loss in the displayed input tallies.

- **Score** = `1.2 × (White points/N / 0.55 − 1) + (Black points/N / 0.55 − 1)`.
- **Decided Games** = `−1.5 × (White draws/N / 0.543 − 1) − (Black draws/N / 0.543 − 1)`.

The reference's preferred result groups have count one and points 0.55. The
standard player aggregation leaves its preferred groups empty and uses the
actual-result fallback groups, which gives the Score formula above. The fixed
0.543 draw fractions and 0.55 point rates are recovered numeric initialization
facts. The native renderer calls its reference GMs, but its players, dates and
sampling provenance are unresolved. These are not statistics recalculated from
the user's database and are not runtime population percentiles.

Native normalized value is `100 × max(0, (raw − lower)/(upper − lower))`. Score
uses bounds `[−1.5, 1]`; Decided Games uses `[−0.5, 1]`. A raw zero therefore
normalizes to 60% for Score and 33.33% for Decided Games. **Values can exceed
100%**: these are composite normalized scores, not fractions of games. Native
plot widths have a separate cap; the displayed percentage is not upper-clamped.

An empty sample produces no score. A missing colour's observed rate stays
unavailable, while the recovered arithmetic's zero-rate fallback is disclosed
and the result is marked partial. Unknown results similarly mark the computation
partial. The report shows the actual numerator, denominator and reference value
for each colour. An OpenFile advisory flags fewer than 20 processed games; that
is a disclosure threshold, not a recovered native significance rule.

Tenacity exposes its verified draw inputs: colour-specific draw fractions,
short draws (stored length below 26) divided by all processed games, and mean
stored draw length. Their fixed reference values are 0.543, 0.13 and 40.1,
respectively. Its composite score remains unavailable because exchange counters
and phase conventions have not been reproduced; the known terms are not summed
into a misleading partial score.

### Categorical grades

The native categorical-grade wrappers read the prior output grade before
normalizing the raw result. Fresh assembly inspection confirms that the first
call's output slot is not visibly initialized and subsequent dimensions reuse
the prior grade. OpenFile does not reproduce undefined stack state or invent a
replacement categorical grade. Cards show **Categorical grade unavailable**.
The raw calculations and explicit mathematical normalization are independent of
that defective gate. The static code contains Very Low/Low/Medium/High/Very High
labels and 25/60/80 cutpoints, but that alone does not establish displayed-grade
parity. No running Windows ChessBase report comparison has been performed.

## Selection, limits and evidence

Player identities are exact database IDs; the user can explicitly add up to 15
additional identities. No automatic alias expansion is claimed. Duplicate entry
matches count once, with White precedence when both sides match unless Black-only
was requested. The report is independent of the active game-list search.

Recovered native selection rules reject deleted entries and stored game lengths
below five and support year and game-category filtering. OpenFile also provides
colour and minimum-opponent-rating filters. All filters run before sampling.
Long games are the default; Blitz/bullet, Rapid and Simultaneous are opt-ins. Categories use verified tournament flags plus native name fallbacks,
with Simultaneous taking precedence over Blitz, Rapid and Long. Unknown category
metadata is excluded by narrower masks; selecting all four categories includes
it and reports its count. That malformed/missing-metadata policy is OpenFile's
explicit choice, not a claim about native file-error behavior.

Year and minimum-opponent-rating filters exclude unknown values that they need.
The sample retains latest valid dates first, then highest entry number. Undated
games follow dated games. Default sample limit is 500, maximum 2,000, chosen
before decoding; skipped records are not silently replaced. The native style
path has no traced sample limit, so this remains an explicit OpenFile resource
limit and the report is not a career-wide or random sample.

Only legal standard-start main lines are validated, up to 400 plies per game.
Setups/Chess960, unreadable, unsupported and partial records are skipped and
counted. Cut records remain marked; result dimensions use the recorded header
result and all processed games, without pretending a truncated position history
provides the missing native style properties. Stored lengths remain distinct
from decoded plies. All analysed entries can be opened. Result-dimension support
links open up to seven games at their last observed legal main-line position;
these illustrate whole-game outcomes, not a claim that the last move caused a
style score.

## Jobs and source validity

The full header scan and replay run outside the UI thread, retaining an
8,192-header chunk and an O(sample-limit) heap. Existing bounded name caches are
used for category metadata. Cancellation is checked before every record's
metadata lookup; within-batch progress is reported every 256 headers and UI
events are throttled to 100 ms. Progress distinguishes selection from replay. Stop
and Close discard pending results; unique generation IDs prevent stale callbacks
from replacing newer results. The WebKit Stop default-submission regression is
retained. Database close/edit invalidates current jobs.

Publication and evidence navigation verify source identities, lengths and
modification times. Evidence loading validates before and after the ordinary
load and closes a stale document. The registry retains only a current watermark
and source signatures, not a report cache. No result/cache is written beside a
source database. The source-signature model does not hash the entire corpus.

## Initial reference investigation (20 October 2026)

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



## Follow-up native investigation (21 October 2026)

Research only. This supplements the initial investigation above, which remains the entry/selection, eight-formula, renderer and workflow inventory. No product code or unrelated research sessions were modified. All results below concern executable SHA256 `8d633b10ad97bd9f8fbbc33a89c279397850ca759c8375093deda4a9485e8dee`, `extracted/update/CBase18.exe`, x86_64 PE. Fresh Ghidra12.1.4 source-immutable import through actual REA MCP. This is static evidence; no Windows execution, runtime report comparison, or engine parity claim.

### Reproduction feasibility

| Native dimension | Formula recovery | Essential inputs available from ordinary decoded game/header? | Remaining barrier |
|---|---|---|---|
| Score | Complete | Yes, actual result and role | Preserve unknown results in denominator; role2 belongs to both colors. Explicit product policy for missing color. |
| Theory | Formula complete; reference mean and diversity now derived | Matched opening code/depth require native board-position taxonomy; selected-position evaluation requires evaluator | Eco.cbcod/Eco.cod or service-supplied compatible taxonomy; exact native evaluation selection. |
| Decided Games | Complete | Yes, draw flag and role | Missing color policy; unknown results remain denominator. |
| Tenacity | Formula complete | Draw/length components yes; two exchange statistics require native exchange predicates and phase conventions | Exchange recognition, static exchange evaluation and move representation not yet independently reproduced. Do not omit these terms silently. |
| Aggressiveness | Coefficients and thresholds complete | Some board properties are reconstructable; essential properties use native position-analysis machinery | Theme detectors, internal evaluation properties, king safety, phase/window semantics. |
| Risk | Coefficients and thresholds complete | Same caveat | Same internal properties; asymmetrical-material detector and king safety. |
| Positional Play | Coefficients and thresholds complete | Same caveat | Same detectors; a raw bishop-pair count is not equivalent to native theme property. |
| Endgame Affinity | Full native formula retained, guards present | Material boundaries and rated result tallies partly recoverable locally | Stable-material transition, evaluation at selected positions, exchange aggregates, exact category populations and availability guard. |

A faithful implementation can expose Score and Decided raw and normalized values now. The other six must identify their unresolved inputs, instead of calculating with guessed substitutes or dropping native terms. This is an implementation assessment, not an assertion that the native behavior cannot eventually be reconstructed.

### Score, Decided and native grade initialization

Direct observations from retained `4a1f20`, `4904e0`, `486ef0` and `49a130`:

* Role0=White,1=Black,2=average. Role2 satisfies both color predicates; result scoring for role2 uses White's result. Actual result0=Blackwin,1=draw,2=Whitewin,3=unknown.
* Counts increment before checking result; unknown results contribute zero points and zero draws, but remain in each color denominator. No games produces native rate0.
* Preferred statistics at profile+b8/d0 are cleared by constructor and not filled by the retained standard `4904e0` aggregation path; its actual-result fallback groups+88/a0 are populated. Reference preferred groups have count1, points.55.
* Raw Score `1.2*(whitePointsRate/.55-1)+(blackPointsRate/.55-1)`.
* Raw Decided `-1.5*(whiteDrawFraction/.543-1)-(blackDrawFraction/.543-1)`.
* Native normalization is `max(0,(raw-lower)/(upper-lower))`, with no upper clamp for displayed percentage. Bounds: Score[-1.5,1], Theory[-2.5,3], Decided[-.5,1], Tenacity[-2,2], Aggression[0,6], Risk[0,2], Positional[0,7], Endgame[0,3].

The categorical-grade dependency is a real ABI-level observation, not only a decompiler artifact. `grade-callsite.asm` shows `4ae240` passes address `[rsp+60]` without an earlier visible initialization; `grade-dispatch.asm` shows `4a86e0` writes raw score at output+8, then tests the preexisting output DWORD before normalization. Raw scorer returns XMM0; there is no hidden result struct initializing this DWORD. First iteration may read uninitialized stack; later iterations reuse previous categorical grade. Do not reproduce undefined stack behavior. Raw scores and mathematical normalization have independent meaning; claiming native categorical-grade parity is unsupported. Threshold wrappers compare25,60,80 after this seed gate. This limitation must be disclosed wherever grades are discussed.

### Theory: opening classification is a position lookup

Direct functions: `48efb0`, `b54800`, `b54a50`, `b54f90`, `b556b0`, `495930`, `44a680`.

`48efb0` calls global classifier `b54800(..., game, 1)`, copies its two outputs into Data+30 and+34. Therefore the **style** branch uses `b54a50`, not the flag0 alternate path. It advances a cloned board to the classifier's maximum ply, then binary-searches compressed board positions and walks backward one mainline move until it finds a match. Returns matched ushort classification code and current board index. Data+34 is a matched ply/index, not stored header fullmoves. Data+30 is not simply header ECOraw. Transpositions can classify by board position.

Loader `b54f90` looks in the native configured directory for `Eco.cbcod`, falls back to `Eco.cod`; reads a two-byte maximum ply and 26-byte records containing a two-byte code and compressed position. The CBCOD variant changes the king nibble in the encoded comparison. Missing-file branch passes an identifier0x35e through `6548f0` to global service `DAT_142295bf0`, callback`b545d0`; there is no evidence that this is a self-contained embedded taxonomy. Service internals and successful missing-file behavior remain unresolved.

Per-color builder `495930` averages Data+34 for roles matching that color or2 and counts each Data+30 key in a support map. `44a680` computes total N (accumulated as integer after each double addition), concentration `sum((count/N)^2)`, then `1/concentration/ln(N)`. Empty/zero concentration returns0; N1 leaves inverse concentration undivided. Raw assembly `theory-diversity.asm` confirms map values are **doubles** at node+28 despite wrong inferred integer casts in pseudocode.

Only named numeric reference statistics were derived; no raw taxonomy, serialized profile, support map or code asset is proposed for product use. `theory-reference-named-statistics.json`: White and Black mean matched ply16.6; both reference support histograms contain496entries totaling93343; their diversity is14.145274396489253. Cohort provenance remains unresolved; renderer calls it GMs. These facts came from `49a130` numeric initializers and independently applying `44a680` arithmetic.

Theory raw formula in prior notes: relative White mean depth +1.5 relative Black mean depth +.2 relative White diversity +.15 relative Black diversity +2 relative White selected-position evaluation -2.5 relative Black evaluation. Both evaluation-reference means.22. Header ECO and existing OpenFile16-ply player tree do not supply the required inputs.

### Theme thresholds and aggregation

Authored `theme-thresholds-support.json` maps all41 verified native property labels to exact strict-greater thresholds and required-positive support counts from `4a3be0` and `4a46b0`. Existing B artifacts hold full Aggression/Risk/Positional coefficient tables; C's `reference-theme-frequencies.json` independently decodes only the needed named baseline frequencies.

`4922d0` scans a bounded position sequence, returns the maximum sliding-window sum divided by window length, and a count of positive property values encountered across the scanned range. Support count is cumulative across that range, not only inside the winning window. If support is below the property-specific requirement, `4a46b0` zeroes a positive property result; required support shrinks to `max(1,trunc(.75*windowLength))` for short windows. Cache is Data+368 +8*(side+2*theme), initialnegative.

`4973b0` provides theme-specific scan ends: ordinary themes end just before first endgame phase; Gambit caps at start+12; pawn-majority, bishop-pair, opposite-bishop and bad-bishop themes can use the last position. Start convention is full-line0 when Data+18 is set; otherwise material-derived Data+50 ifpositive, else min(20,max(0,endBeforeEndgame-1)). Do not conflate Data+34 matched opening depth with Data+50 material phase boundary.

`498c90` is a41-way property switch, many values call native board property accessors and analysis structures. Ghidra incorrectly types it and its lambda as returning void, losing XMM0 expressions in the pseudocode. Its retained pseudocode is evidence of dependencies, not a complete trustworthy implementation specification. `48fd70` copies numerous values from internal analysis object via`21ef30`, `21f400`, etc.; `21ef30` dispatches through a per-side object's virtual+10. Missing return types and internal implementations remain barriers. King attacks, sharpness, calmness, successful defense and piece strength must not be invented from similarly named strings or general chess intuition.

### Scalar fields used by the remaining dimensions

Define L=`4975a0` analyzed length. Native endBeforeEndgame is firstphase-1 when1<=firstphase<decodedPositionCount, otherwise last retained position. If full-line flag is true, start0; otherwise Data+50 whenpositive, else min(20,max(0,endBeforeEndgame-1)). L=endBeforeEndgame-start+1. This is a **position/ply interval**, separate from stored header length.

Direct mappings from `4904e0`, callbacks, `48d4b0`, observation emitters:

| Profile field | Exact numeric source | Semantic certainty |
|---|---|---|
|+320|10*mean(Data+5c/L), zero if L<=0; callback4a5700|Exchange-related counter; detector details unresolved.|
|+328|10*mean(Data+60/L); callback4a58c0|Exchange-related counter when3c4b80 succeeds on relevant side.|
|+330|10*mean(Data+64/L); callback4a5970|Exchange-related counter for remembered move matching.|
|+338|10*mean(Data+68/L); callback4a57a0|Counter when45b780 detects a move affecting exchange opportunity.|
|+340|Mean per-game own-side mean of nonzero per-position+90 before endgame;494b40|Native observation labels prove king-safety requirement: lower=not worried about king safety, higher=needs king safety. Underlying virtual property still unresolved.|
|+348|Same for opposite side|Opponent king-safety property.|
|+308|Count Data+78>=0 / all games|Native observation labels prove asymmetric-material frequency;3e3660 uses material signatures, balanceabs<=100 and persistence3e3960, not a generic unequal material count.|
|+310|Among games L>10, fraction where499e50>0.6L|Queenless-interval frequency; low yields native label 'Keeps Queens on Board'.|
|+370|Predicate-frequency from499f30; selected predicate4a5640|Short win near endgame phase, not result-point rate. See original predicate guard.|

`3e4290` sets Data+7c to first position at or after Data+50 where both sides' queen counts are zero; else-1. `499e50` measures the interval from this queenless marker to endgame (or line end), only if marker precedes endBeforeEndgame-1. Thus profile310 is not 'queen exchanges per move'.

`48d4b0` weights exchange-related counters by integer pieceValue/100. Piece codes table:0=0,1=king15000,2=queen900,3/4=bishop/knight325,5=rook500,6=pawn100. `3c4b80` clones a board, generates legal capture candidates, requires equal piece type and static exchange score0 from`b429f0`, and tests original move opportunities. `45b780` tests whether a subsequent move prevents/changes such an exchange opportunity, including ray-interposition geometry. Exact counter names would be an inference; retain numeric definitions until independent fixtures verify.

`499f30` returns (predicateCount, predicateCount/allGames, performance). Its second field is frequency; this settles+370 meaning. Ghidra loses some performance call parameters; use assembly when implementing.

### Endgame boundaries and evaluation

`3e4120`: scans material history backward from the final position for last position with nonpawn material total>3100 and some bishop/knight/rook material; queen900,minor325,rook500. It then calls `3e3850(history,index,1)` and caches firstphase. A simple 'material <=31' first encounter does not match this backward stable-transition convention.

`3e4860`: scans backward for last position where either side has>1 nonpawn piece, then calls the same stable-transition helper, caches technicalphase and returns material-balance sign from that phase. `3e3850` searches subsequent equal material-signature entries while accounting for a history flag and has fallback branches; exact flag construction remains unresolved.

`48f430` classifies an endgame only if firstphase>=1 and firstphase<decodedPositionCount-20. Evalabs<50 means equal; abs>=71 classifies favorable/worse with role sign;50..70 gap and no qualifying evaluation fall back to actual result as in prior evidence. Technicalphase predicates `48c530/48c5a0` require enough remaining positions (>11) plus balance sign. Do not substitute a fixed move40 or generic queenless boundary.

`b55f40` exact evaluation contract:

* If typeword!=-1, use low3bits. Kind0 or3 returns signed16 payload. Kind1 maps mate distance d>0 to30000-d and d<=0 to -30000-d. Kind2 uses28000 instead. Otherkind returns32767 sentinel.
* Typeword==-1 uses W/D/L counts. If total<1 return32767. p=(wins+.5draws)/total. p<=0 returns-30000; p>=1 returns30000. Otherwise lround(200*log10(p/(1-p))), clamp[-25000,25000].
* Native evaluation-side normalization occurs elsewhere; this accessor alone does not prove centipawns are White-relative or bound handling. Existing UCI helper discarding bounds and mapping all mates±30000 is not a faithful typed adapter.

### Rated performance and relative endgame scores

`48e1c0` and `c49250/c492c0` recovered, with `performance-helper.asm` verifying surprising smoothing placement. N=rated subset count; O=sumOpponent/N; A=sumOwn; S=subset points. Constructor stores countN+1 and int64(O+A). Query percentage argument is `100*(S+.5)/N +1`, **not** `100*(S+.5)/(N+1)`. Helper computes delta400log10(p/(1-p)), p=percentage/100, endpoints±800, then int((O+A)/(N+1)+delta+.5) when rating sum nonzero. Returned performance=.5*(helperInteger+O). N0→0. The +.5 cast is C++ truncation, not generic round-to-nearest for negative values; separate specialnegative handling exists only when storedratingSum0.

`4a1f20` includes only games with both ratings nonzero in this performance subset. It also calls a rating-bound helper with one rating limited±400, but visible sumOwn/sumOpponent use the stored Data rating fields; do not assume an existing FIDE DP table is equivalent. For complete caller ABI validation retain original assembly and summed-rating inputs.

`4a6a50` compares player/reference rates with multiplier and positive-only option. When its last flag istrue it first divides each by respective overall point rate, choosing preferred overall group+e8 ifcount>0 elsefallback+70 and reference counterpart. It requires nonzero denominators for the adjustment. This is not a raw win fraction difference.

### Tournament category bytes

Native selection predicates in B use canonical CBTournament+3c: Simul low5bits7/namecontains' sim', Blitz highbits0x20/nameblitzorbullet, Rapid0x40/namerapid, Long neitherBlitznorRapid. PrecedenceSimul,Blitz,Rapid,Long.

`1dc0f0` writer and `1db310` constructor prove classic90-byte name payload: dateLE32 at+0x46, flagsLE16 at+0x4a. Existing classic99-byte CBT records include9-byte treeprefix, giving record date79..83 and flags83..85. Constructor reads signed16; bit tests onlowbits areunchanged. This constructor does **not** apply legacytype5 remap.

For full stream reader `1ddba0(param3=1)`, sequence is siteUTF8string,eventUTF8string,4-byteLEdate,LE16flags,LE16nextfield,byte,byte,LE16nextfield. `f075c0` date wrapper ultimately`f08c60` copies4rawbytes and suppresses values<=0x220. `f07ac0` copies2rawbytes. Low5type5 in stream reader is remapped to `(flags&~31)+0x103`. Same remap exists in compact legacy reader1ddec0. Existing2LID names implementation uses exactly the two leading strings and currently ignores tail. Linking this full stream variant to every2LID producer/version still needs a fixture roundtrip; the byte sequence and remap are direct native facts, file-format mapping is a strong inference based on matching names layout.

### Evidence index and residual unknowns

Each named request JSON contains complete REA result and evidence ID. Key batches:

* scalar-callees: ev_43af4d0633f734b046e595b5c09ae300d3532434ac4d2b23ff00a9d1d48377be
* theme-inputs: ev_d650eee57c387375b0be302d147f52e56a972aba524af89d22de364bd8243420
* phases-and-eval: ev_61337a2ce1c503a2b2fe8bf654ae911b5f0e3ec762a0ee4379b9c1dfa257771b
* endgame-and-metadata: ev_b5de90d56c3060508dcd1e32845554f1434684d1d6af4a09890cb837de9778cf
* opening-source-tournament-fields: ev_b0b509f83444eeb921548369d69200ff0c5275a46b8a5e1b45abc7d44aab6e0c
* style-opening-fastpath: ev_d9cbaa6096b35915cd76e915a3d0648d7e199d678abc998edf63eb05c8c58c3a
* exchange-scalars-queen-marker: ev_bf7cc1903c9d2a4442114921c524901023d53023b936695ffed5c39db0238d1d
* tournament-serialization: ev_b25603065f54e15513e85043173aed876a8727998e872a273497d4dd0601478d
* tournament-final-byte-contract: ev_372ebf875975795320b4d5a4ec3700f28fb07ab45541b566c51abdb0df9b80bc
* classic-tournament-constructor: ev_02ba8b05e6287d0f38c97738774ca5274a82ecf2b374bac492b3a5afa38e5ab0

All fresh function pseudocode in `pseudocode/`; raw assembly independently saved for grade ABI, constructor fields, diversity and performance. Research notes separate exact numeric facts from inferred semantic labels. The full source functions remain available for later refinement.

Unresolved: GM reference population dates/selection and reproducibility; exact native engine selection/version/search settings and bound handling; missing-opening-table service protocol; internal theme-property calculations/return types; stable-material history flag construction; exact exchange predicate fixtures; full2LID variant coverage; categorical-grade seed behavior under actual runtime compiler stack state. No strings-only or mathematical-formula-only evidence establishes native score compatibility for the six unavailable dimensions.


## Native acceptance and performance

The replacement was verified in the rebuilt macOS Tauri app against read-only,
byte-identical temporary clones of Mega Database 2025 (11,151,119 entries).
Exact name-list identities and nearby entries were checked: **Carlsen, Magnus
#65470** and **Nakamura, Hikaru #286135**. No aliases were added; this is not a
claim of exhaustive alias discovery. Both reports use both colours, Long only,
no year/rating filter, latest 500 eligible games and the 400-ply replay cap.

| Observed result | Carlsen | Nakamura |
|---|---:|---:|
| Eligible before sampling | 2,056 | 1,814 |
| Sampled / analysed | 500 / 492 | 500 / 483 |
| Skipped setup games | 8 | 17 |
| White games / wins / draws | 247 / 117 / 115 | 246 / 90 / 141 |
| Black games / wins / draws | 245 / 68 / 157 | 237 / 49 / 152 |
| Analysed date range | 2016.09.12–2024.09.21 | 2015.10.19–2024.06.07 |
| Score raw / normalized | 0.4286044782 / 77.14417913% | 0.1824599811 / 67.29839924% |
| Decided Games raw / normalized | 0.0337073788 / 35.58049192% | −0.2644683398 / 15.70211068% |
| Short draws / all processed | 15 / 492 | 31 / 483 |
| Stored draw-length sum / draws | 12,702 / 272 | 11,985 / 293 |

Neither sample has unknown results, unreadable/unsupported/partial skips or a
400-ply cut. These are recent **Long-category samples**, not career totals. The
span reflects how often the selected player has eligible Long games in this
corpus; it is not a fixed-length time window. Long does not itself exclude
Chess960/setup records; those are subsequently skipped by OpenFile's replay rule.

An independent Python validator read the classic 46-byte headers and 99-byte
CBT tournament rows directly, reproducing category precedence, deletion,
stored-length filtering, eligible counts and the latest-500 selection. Separate
python-chess 1.10.0 PGN/result replay matched both colour populations, both raw
and normalized scores, Tenacity's known draw inputs, every supporting SAN/ply
and each analysed game's replay length. Six composite scores and all categorical
grades were confirmed absent. The independent export and raw-data checks remain
outside the repository; no private game collection is distributed.

Native supporting links opened **Carlsen–Schitco**, entry 11131908, at **42.g5**
(ply/node 83), and **Praggnanandhaa–Nakamura**, entry 10932339, at **31.Rxe8+**
(ply/node 61). The expected board, highlighted move and source entry were
inspected. These whole-game outcomes support Score/Decided Games; no causal
claim is made about the last move. Stop was exercised during the full header
scan, remained stopped without publishing a partial report, and has both the
WebKit click-default and within-batch cancellation regressions.

### Native UI evidence

All images below are actual app captures, visually inspected. They replace the
superseded ten-metric screenshots. Only two cards contain computed scores;
missing calculations remain visible throughout.

- Carlsen: [sample and scores](media/style-native-carlsen.png),
  [result inputs and Decided Games](media/style-native-carlsen-inputs.png),
  [unavailable composite dimensions](media/style-native-carlsen-unavailable.png),
  [supporting position](media/style-native-carlsen-evidence.png).
- Nakamura: [sample and scores](media/style-native-nakamura.png),
  [known draw inputs](media/style-native-nakamura-tenacity.png),
  [unavailable composite dimensions](media/style-native-nakamura-unavailable.png),
  [supporting position](media/style-native-nakamura-evidence.png).
- [Stopped native scan](media/style-native-cancelled.png).

### Performance and verification

Warm-cache release runs over all 11,151,119 headers took **0.408 s** for Carlsen
and **0.351 s** for Nakamura: selection 0.189/0.159 s and replay 0.218/0.191 s.
Prepared name-list lookup took 0.003/0.002 s. Whole validation processes,
including independent PGN export, peaked at **16.5/15.9 MiB RSS**. The first run
also built the name index: lookup 0.461 s, whole-process RSS 71.3 MiB. These
measurements exclude initial database opening and filesystem-placeholder delays;
they are representative measurements, not latency guarantees. Engine cost is
zero. Native debug UI computations took 3.14/3.32 s, as recorded in the screenshots.

The nine original database-family sizes, modification times and inodes remained
unchanged. Original and clone CBH SHA-256 both equal
`1750ba956d38339091392c7adea9e26bb2cfce1f1438d876fdeb76d8e0789845`.
Original checkout changes and unrelated research sessions were preserved.

Final local checks: **342 Rust tests**, **196 frontend tests**, formatting,
strict workspace/all-target Clippy, TypeScript, Prettier and production frontend
build pass. Locked native release and separately identified embedded-frontend
acceptance app builds pass. The existing frontend bundle-size advisory remains.
Focused independent review found a metadata-scan cancellation delay and an
attribution overstatement in the selection docs; both were fixed, with no
residual actionable concerns. The distribution boundary audit passes for all 661 tracked paths.
The native coverage checklist remains **partial**.
