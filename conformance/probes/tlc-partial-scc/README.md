# Bounded partial SCC and graph-growth controls

Tracking #185 / #66, ADR0048. This new family preserves all eight obligations
at four independent actual source prefixes (`selection.json`,32 rows), plus
eight repeated original `LiveCheck.check(force=true)` runs on growing graphs.
These are explicit prefix projections, not ordinary wall-clock schedule runs.
A driver process exit0 is not a model verdict: `partial_check_code` and original
message streams record each actual check; successful partial checks do not mean
holds. `PartialLater` has codes0/0/0/2116 across prefixes0/1/2/3;
`PartialGrowthSafe` has0 throughout successive actual checks on the same graph.

The inspector invokes pinned private non-final `LiveWorker.checkSccs`, actual
`checkComponent`/postfix routines and original path reconstruction, retaining
roots, done/undone registers, ordered adjacency, component pop order, anchors,
selected postfix/prefix and original configured/derived labels. Its observing
copy is a field projection, not identical disk-offset/write-history evidence.
The original graph then runs unmodified `LiveCheck.check(force=true)` separately.

The repeated driver stops on the first actual candidate. The native diagnostic
intentionally continues later source commits to expose graph growth; this
continuation earns no ordinary stop/schedule/count or complete-model credit.
All originally selected rows remain present. `development-repeated-launcher`
preserves eight class-not-found failures from an incorrect compiler source list;
those rows contain no runtime check evidence and are not silently replaced.

Focused production controls now match128 full prefix projections (32 rows at1/3/4/8 workers),32,768 independent graph/write-mask acceptance cases, and fresh periodic-word/memory/JSON witness mutants. Private source-slot provenance and shared-budget repeated-growth negatives pass. The closed full gate passes all35 stages and965 tests with0 skips. The actual standalone Cargo caller exits0; CM121 and validation-smoke16 all agree at1/4/8 workers. `gate-current` retains the complete logs/receipts; `development-stale-xtask` retains the first gate's stale removed-checkout xtask failure after all965 tests passed.
Existing graph entry/cell bounds stay fixed. Partial checks share the existing
Observer graph.steps quota with original graph insertion/recursive inspections;
Session predicate/value limits are distinct and unchanged. Charged reservations
and completed selection work are reported separately from actual model counts.
Failing SCC-stage work is not invented from a lower bound. No aggregate byte or
memory accounting, periodic clock, exact-store temporal proof, host/effectful
execution, ordinary strict liveness or family certification is claimed.
