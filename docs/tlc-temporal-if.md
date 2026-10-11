# Temporal conditional identity repair (#170)

Bounded explicit final/TLC checking now admits the proven direct temporal-IF
subset. This is available through `--compat=tlc --lncheck=final --lasso=tlc` and
`dive_api::tlc_temporal::check_final`. It does not implement ordinary periodic
scheduling, upstream-refused temporal argument forwarding, or accepting-sink
early witness reconstruction. No default or resource ceiling was changed.

```tla
Live == [](IF x=0 THEN x=0 ELSE x=1 => <>(x=2))
```

On the pinned build, the IF guard is one shared liveness object. The tag walk
visits it twice and assigns its final tag, 3, to both references. Tag 1 becomes
an unused slot. The earlier Rust importer instead allocated tags 1 and 3 to two
independent objects. This changed initial tableau particle numbering (3 versus
4), even when truth on tested words matched. The original source, failed
assertion and raw captures remain in the [probe directory](../conformance/probes/tlc-temporal-if/).

The corrected importer preserves input arena-node identity, updates earlier
references with the last assigned tag, and preserves every visit/unused slot.
Separately constructed predicates with equal truth or equal expression text
still have distinct identities. Every rewrite consumes the existing work
budget; map entries and reference arrays are bounded by charged import terms,
not by a claimed aggregate memory/byte bound. No limits were increased.

All 25 selected reference invocations remain recorded: 20 complete diagnostic
normalization/group/tableau projections and five failures. Selected nested,
INSTANCE-body, multiple-property, constant and distinct-object cases retain
exact bins, particle identities and ordered edges. 5,076 independent periodic
word/closing-position controls across 18 pure models preserve truth; this is
language evidence, not a model certificate.

Two temporal-formal forwarding cases (`TiLet`, `TiForward`) are still refused
at final admission: pinned TLC rejects them with 2213 while diagnostic Rust
lowering expands them. That diagnostic gap receives no field/error agreement
credit. Three clean reference cases (`TiThen`, `TiNested`, `TiMulti`) stop early
with message 2116 and exit 12, so a completed helper graph cannot certify their
actual stopping/error contract. All 25 clean outcomes and the separate 15
completed/10 failed final-projection controls are retained without promotion.

## Run a temporal conditional

From the repository root:

```sh
cargo run -p dive-cli -- check \
  conformance/probes/tlc-temporal-if-final/models/IfTableau/IfTableau.tla \
  --compat=tlc --lncheck=final --lasso=tlc --seed=0 --fp=0 \
  --workers=4 --output=json
```

The example reports a liveness violation with exit 13, schema 2, actual worker
width and exploration statistics, and an independently validated concrete
counterexample. `execution.schedule` is `final-only`; the JSON explicitly states
that ordinary temporal scheduling is unsupported. A bounds/refusal result has
no validated witness and never certifies success.

For Rust callers, use the executable
[API example](../conformance/probes/tlc-temporal-if-final/user-example.rs):

```rust,ignore
let report = dive_api::tlc_temporal::check_final(
    &ir, &config,
    &dive_check::CheckOptions { workers: 4, ..dive_check::CheckOptions::tlc(0, 0) },
    dive_liveness::phase_graph::Limits::default(),
)?;
```

`ir` comes from `analyze_file` and `dive_ir::lower`; the config comes from
`dive_config::parse`. The linked program loads both from the repository example,
checks the private validated outcome and prints its original configured claim.
The API never accepts a caller-provided graph or a claimed validated flag.

## Exact scope and limits

The complete current selection retains all 30 models: 25 completed diagnostic
normalization/particle projections and five reference failures; actual final
helper selection retains 20 complete rows and ten failures. Across widths
1/3/4/8, the native CLI retains all 120 outcomes: 76 independently validated
violations, four holds and 40 named refusals. The supported 20 models match
actual final component/anchor/bin/word fields; none of the ten refused models
receives error agreement credit. Five new compound guards add AND, OR, NOT,
temporal guard and nested branches. Separate independent finite-word/closing
checks cover 6,486 comparisons across 23 current pure models; the earlier 5,076
comparisons and original failed observations remain unchanged.

Configured claim `Live`, INSTANCE action identity, and derived role `I!Prop`
are checked separately. Formula truth or state-count equality cannot substitute
for correct property attribution or exact tableau identities.

The remaining forwarding restriction scans the whole admitted arena, including
unused calls. Passing a temporal expression into an IF-bearing user body is
refused, even when unused, because selected pinned examples fail with 2213.
Action-guarded temporal IF remains a planner refusal (2214). Structural checks
follow actual expression/definition edges rather than trusting erased metadata;
opaque/effectful captures and invalid contexts retain their existing refusals.

The unchanged 4,096 formula arena-entry/structural-admission work ceiling and
all existing formula, tableau, capture and product ceilings still apply.
Large unused/imported arenas can therefore refuse. Entry/work bounds are not an
aggregate byte or memory cap; a capture-state limit constrains retained graph
representatives, and actual stopping counts may include the deciding inserted
state. No incomplete capture or exact-store fingerprint collision earns success.

Three cases (`TiThen`, `TiNested`, `TiMulti`) stop in TLC's source prefix with
message 2116 and exit 12. Native final mode still explicitly refuses accepting-
sink early witness reconstruction; their completed helper projections never
receive final verdict credit. Original source-prefix exits remain a separate
implementation obligation.

See the [full final contract](tlc-final-api.md),
[pure state conditional guide](tlc-state-if.md), and
[raw/source evidence](../conformance/probes/tlc-temporal-if-final/).
Full temporal families and ordinary periodic schedules remain partial.

The frozen temporal-IF producer's 140-model small-corpus study has
6 agreements and 134 named refusals, with no disagreements or timeouts. All 138
actual native invocations are retained; two worker-introspective models refuse
before invocation. This is no corpus-coverage gain over the pure state-IF slice.
The first study launcher omitted its raw parent directory and failed before any
native model ran; that complete failed report is retained separately and earns
no semantic disagreement or agreement credit. The full gate also preserved its
initial stale CLI refusal expectation for `IfTableau`; the corrected positive
is paired with an explicit exported-forwarding CLI refusal at every width.


## Published integration verification

The combined conditional/scoped final implementation passed all 957 workspace
Rust tests, with none skipped, and every local gate stage. Fresh CLI checks at
1, 3, 4 and 8 workers reproduced all 120 selected IF outcomes: 80 completed
final outcomes and 40 named refusals. Both the IF and scoped-property Rust
examples compiled and ran through Cargo against the same frozen source.

All 21 CM1 and 16 validation smoke models agree at 1, 4 and 8 workers, including
the smoke selection's unchanged 30-second CI limit. The
[source-bound receipt](../conformance/probes/native-parity-integration/current-controls/scoped-temporal-publication/verification.json)
retains every current result and the earlier failed source-binding gate.
These selected final-only checks do not certify periodic scheduling or a full
feature family. The historical 140-model census above was not rerun on this
combined source and earns no new corpus-coverage credit.
