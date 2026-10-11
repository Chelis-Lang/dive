# Scoped temporal properties in explicit final mode

Pure expression arguments can retain their caller context through direct and
local calls, nested LET definitions, nested INSTANCE bodies and substitutions,
and supported ENABLED formulas. This bounded slice is available with
`--compat=tlc --lncheck=final --lasso=tlc` or
`dive_api::tlc_temporal::check_final`. It does not implement ordinary periodic
liveness scheduling or unrestricted higher-order, recursive or exported-formal
INSTANCE semantics. No resource default changed. Directly syntactic `~ENABLED A` now preserves
TLC's whole state-atom shape instead of an extra structural negation. The action
stays pure and bounded; opaque/nested unsupported actions remain refusals.

```tla
Identity(p) == p
Relay(p) == LET Saved == p IN Identity(Saved)
P == Relay([]<>x)
```

A zero-arity LET definition and a formal expression argument both use retained
lazy contexts internally, but TLC gives them different attribution rules. TLC's
`processConfigProps` renames a derived role when it resolves the local definition
`Saved`, and preserves the previous role when it unwraps a formal argument.
The binder now preserves this distinction. Earlier native `Identity` versus
pinned `Saved` mismatches and the exact failed source remain in the
[probe history](../conformance/probes/tlc-final-scopes/development-derived-role-mismatch/).

## Run and replay a forwarded property

From the repository root:

```sh
cargo run -p dive-cli -- check \
  conformance/probes/tlc-final-scopes/models/TfClaimPairFails/TfClaimPairFails.tla \
  --compat=tlc --lncheck=final --lasso=tlc --seed=0 --fp=0 \
  --workers=4 --output=json
```

The two configured properties `P` and `Q` pass temporal expressions through
`Identity`. This run reports exit 13 and schema 2, a final-only schedule, actual
requested/used workers and counts, and a freshly independently validated
counterexample claiming `P` and `Q`. They are original claims, not the derived
labels `Identity;Identity`. The witness is validated both in memory and after
JSON decoding against a fresh bound model. False initial states, invalid closing
indices, malformed closing states, empty/foreign claims and substituted derived
role claims cannot acquire a valid certificate.

The [executable Rust example](../conformance/probes/tlc-final-scopes/user-example.rs)
loads the same source and config and calls:

```rust,ignore
let report = dive_api::tlc_temporal::check_final(
    &ir, &config,
    &dive_check::CheckOptions { workers: 4, ..dive_check::CheckOptions::tlc(0, 0) },
    dive_liveness::phase_graph::Limits::default(),
)?;
```

It prints `["P", "Q"]` after inspecting the private validated outcome. Use
`analyze_file` for nested modules, then `dive_ir::lower` and
`dive_config::parse`; do not replace retained contexts with a second evaluator.
The API accepts source-bound inputs, never an arbitrary graph or a validated
flag. See the [final API guide](tlc-final-api.md) for exact bounds and errors.

## Three names, three separate contracts

* Configured properties are the original names before splitting: `P` or `P;Q`.
* Original expanded action names come from TLC's `Action` vector: splitting a
  conjunctive `P` can produce `P;P`.
* Derived role labels come from TLC's implied-temporal name vector: source spans,
  `Saved`, `Identity` or the resolved INSTANCE name `J!Claim`.

Native planning/execution currently expose the first and third fields. They are
compared exactly with the pin. Java's expanded action-name vector remains a
reference-only obligation; this slice does not invent that missing native field
from a configured spelling. Equal formula truth, counts or witnesses cannot
substitute for correct field attribution. Issue #134 remains open for that
representation obligation.

## Evidence and restrictions

The complete selection retains 18 fixtures. Sixteen normalization/tableau
projections complete, two exported INSTANCE temporal-formal fixtures fail.
Fourteen fixtures have actual temporal obligations; two state-only properties
route through ordinary bounded safety and never claim a temporal product.
The forced Java final helper also checks fairness for those safety inputs, but
those diagnostic invocations earn no actual temporal verdict credit.

The two exported-formal fixtures are refused by name; clean pinned TLC reports
2213/exit255. That refusal is not an exact error-class agreement. Impure, opaque,
action-kind, invalid-context and unsupported forwarding cases keep their
existing named refusals. In particular, temporal expressions forwarded into an
IF-bearing user body remain conservatively refused across the whole arena,
including unused calls. Bounds may refuse large unused/imported arenas too.

The existing 4,096 formula-entry ceiling, structural admission work quotas and
formula/tableau/capture/product limits remain unchanged. These are not aggregate
memory or byte bounds. A capture-state limit counts retained representatives;
engine counters can include an already-inserted deciding state. Resource stops
and fingerprint collisions never certify complete exploration.

The multi-property controls also preserve TLC's singleton disjunction factor
when `LNConj.toDNF` has a Cartesian cardinality of one. Its later flattening is a
separate stage. The original shape mismatch is retained alongside the repair.
All selected failures, prior source manifests and raw observations remain
available. None of this certifies ordinary scheduling or a full feature family.

The frozen scoped-property producer passed all 923 workspace tests with none skipped, plus the
complete local gate. The ordinary safety controls remain green: all 21 CM1 and
16 ValidationTestSuite smoke models agree at 1, 4 and 8 workers. These receipts
are [retained with the fixture evidence](../conformance/probes/tlc-final-scopes/gate-current/);
they do not promote the historical full-corpus census or certify a whole family.


## Published integration verification

The combined implementation passed all 957 workspace tests with none skipped,
plus the complete local gate. Fresh runs retained all 72 scoped CLI outcomes
at 1, 3, 4 and 8 workers: 56 temporal outcomes, eight state-only safety outcomes
and eight exported-formal refusals. Both exact Rust guide examples compiled and
executed through Cargo. Property claims, derived roles, pinned verdict/count
fields and independent witness validation were checked separately.

The ordinary safety gates also fully agree on all 21 CM1 and 16 validation smoke
models at 1, 4 and 8 workers. The smoke selection retains its 30-second CI limit.
Read the [combined-source receipt](../conformance/probes/native-parity-integration/current-controls/scoped-temporal-publication/verification.json)
for source identity and retained raw results. Expanded original Action metadata,
ordinary periodic scheduling and broad-corpus certification remain separate
implementation obligations.
