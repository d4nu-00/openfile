# Quick Search Expressions

The game-list quick filter should answer common database questions without opening
the full search mask. Preserve its existing plain-word behavior and its fast scan
of entry headers. Add compact terms for player colour, event/site, year and ECO
ranges, average Elo thresholds, and exclusion. Terms combine with AND, except
positive result terms, which form one OR group. A `!` prefix negates one term.
Missing ratings do not satisfy rating comparisons.

Examples: `Kasparov- 1985`, `-Karpov B10-B17`, `Site:London !1/2`,
`2001-2011 >2400`, and `Event:"World Championship"`.

The input should show a short syntax hint. Malformed expressions must produce a
clear search error rather than silently return misleading results. Results stay
in the existing list and retain its current cancellation, paging, sorting and
two-million-hit limit. This is OpenFile query syntax informed by the manuals;
it does not claim to clone either application's parser.

The reference executable's list-filter command and its search-mask behavior are
being inspected through REA with Ghidra. Native findings will be recorded in
`docs/search.md`, with uncertainty stated rather than inferred away.
