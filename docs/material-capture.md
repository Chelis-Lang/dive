# Captured concrete values in final checking

Explicit bounded final TLC checking now admits stored strings, model values,
tuples, records, general functions and explicit sets captured by a split action
or assigned to a constant. This fixes a native refusal; it introduces no new
TLC divergence or default. The capture proof uses the actual bound context,
not an `OPEN` annotation or a string spelling as purity authority.

For example:

```tla
---- MODULE Material ----
EXTENDS Naturals, TLC
CONSTANT a, b
VARIABLE x
Init == x = FALSE
Next == \E v \in {<<[tag |-> "ok", vals |-> {a,b}], <<1,2>>>>} :
          v[1].tag = "ok" /\ x' = x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == []<>(x = TRUE)
====
```

Use a `.cfg` containing `SPECIFICATION Spec`, `PROPERTY Live`, and
`CONSTANTS a = a` followed by `b = b`. The tracked executable fixture is
`conformance/probes/material-capture/models/MatNestedFails/`:

```sh
cargo run -p dive-cli -- check \
  conformance/probes/material-capture/models/MatNestedFails/MatNestedFails.tla \
  --compat=tlc --lncheck=final --lasso=tlc --seed=0 --fp=0 --workers=4 --output=json
```

It reports a schema-2, final-only liveness violation with a freshly validated
one-state stuttering counterexample, `Live` as the original claim, 2 generated
states and 1 distinct state. The pinned single-worker TLC gives the same word,
ending and counts. Requested/used worker facts describe this native run; they
are not copied from the reference. The holding pair toggles `x` and completes.

## Rust callers

Use fresh source inputs and the final facade; callers cannot provide a graph or
set a validated flag to obtain a certificate. This complete executable example
is also tracked as `conformance/probes/material-capture/user-example.rs`:

```rust,no_run
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(
        "conformance/probes/material-capture/models/MatNestedFails/MatNestedFails.tla",
    );
    let analysis = dive_sema::analyze_file(path, &Default::default())?;
    let ir = std::sync::Arc::new(dive_ir::lower(&analysis).map_err(|e| format!("{e:?}"))?);
    let cfg = dive_config::parse(&std::fs::read_to_string(path.with_extension("cfg"))?)
        .into_config().ok_or("invalid configuration")?;
    let options = dive_check::CheckOptions {
        workers: 4, ..dive_check::CheckOptions::tlc(0, 0)
    };
    let report = dive_api::tlc_temporal::check_final(
        &ir, &cfg, &options, dive_liveness::phase_graph::Limits::default(),
    )?;
    if let dive_api::tlc_temporal::Outcome::Violation(v) = report.outcome() {
        assert_eq!(v.validation().validation, dive_trace::Validation::Valid);
        println!("{:?}", v.document().properties);
    } else { return Err("expected validated violation".into()); }
    Ok(())
}
```

## Admission and remaining refusals

The non-evaluating capture proof first requires the model IR, interner, roles
and scopes to belong to the admitted evaluator. Named values and record keys
must resolve to the same token/text in that interner. Stored Enum/Raw sets,
records and general functions must also have the evaluator's value order.
Same-owned atoms alone are insufficient: a set cached in canonical order can
change TLC-mode `CHOOSE` despite its correct interner. A mismatched supplied
value refuses before pool work, counters or certification. Tuples are exempt
from the outer storage-order check because their implicit domain is ascending
integers in either profile; nested records/functions/sets still must match.
A concrete stored function
can be inspected recursively; it is not an opaque function closure. The proof
reads only explicit Enum/Raw set storage. It does not normalize, compare, hash,
fingerprint, test membership or call a deferred provider to guess purity.

The existing ceiling is **50,000 inspections per predicate and depth 128**.
Expression visits and recursive values share that predicate budget; function
keys, including implicit tuple-domain integers, and ranges each consume work.
Exhaustion keeps the named capture admission refusal with no started worker or
proof. These bounds do not promise aggregate allocation, bytes, process memory,
or that all predicates together fit within 50,000 inspections. Final graph and
product limits remain independently enforced as documented in the
[final API guide](tlc-final-api.md).

A deferred set stays deferred even if its elements were previously cached.
Intervals, products and other computed set representations are not stored Enum/Raw
captures in this increment. Known nonrecursive user operator captures have a
separate bounded proof in [the operator guide](temporal-operators.md). Opaque
operator/formal captures, configured value overrides of definitions, unresolved
context operators, `Unbound`/`Unevaluable`
leaves, impure/live/runtime-name-producing operators and unknown IDs remain
refused. Constructing or hashing a composite value can already run deferred
provider code before admission; this change guarantees no additional provider
calls during its inspection, not safer arbitrary-value constructors.

`OPEN` means a definition might depend on configuration. The proof resolves
actual constants and lexical/substitution contexts without evaluating them;
unresolved or opaque branches still refuse. Static effect flags are unchanged.
The global compatibility/profile rules, ordinary schema 1, approved defaults
and host grants are unchanged.

The 18 clean paired reference fixtures and 72 native runs at 1/3/4/8 workers
compare counts, selected words/endings, original claims and fresh independent
memory/JSON replay. Mutated initial/closing states, claims and endings refuse or
invalidate. Direct, cached and nested opaque providers add zero admission
callbacks; ownership, name-token collision, work and depth controls remain
negative, including opposite-profile stored values and matching tuple domains.
The pre-order-guard 72 positive rows and interrupted 931-test gate are retained
as blocked development evidence, alongside the public same-owned-string
`CHOOSE` counterexample; they receive no current producer credit.
Earlier 32 original and 4 supplementary native refusals, and the first partially successful 64-row
attempt remain separate historical evidence. The earlier baseline census was **6 agreements / 134 unsupported**. A complete
frozen material-producer census selected all 140 small-tier models with the
unchanged 600-second per-model bound: **46 agreements, 92 unsupported and 2
timeouts, with no disagreements**. Both timeouts and the nonzero harness exit
remain recorded; the original launcher-receipt mismatch has a separate closed
consumer addendum. That census belongs to the material producer, before the
operator integration, and cannot certify this combined source. Ordinary periodic
scheduling and full temporal-family certification remain pending. Developer source/evidence: `conformance/probes/material-capture/`
and `crates/dive-check/src/capture/pure.rs`.
