# Pure native temporal operator arguments

The bounded final TLC mode accepts `Len`, `Head`, `Cardinality`, and fixed-arity `Append` as directly
supplied operator arguments, including pure wrapper, relay, `LET`, and named
`INSTANCE` contexts. Native authority comes from the admitted IR's `Def.std`
metadata or an explicit `StdOp`, never the operator's spelling. Ordinary periodic
liveness scheduling remains unsupported.

For example:

```tla
---- MODULE NativeHeadFails ----
EXTENDS Sequences
VARIABLE x
Init == x = <<FALSE>>
Next == UNCHANGED x
Use(F(_)) == []<>F(x)
Live == Use(Head)
====
```

With `INIT Init`, `NEXT Next`, and `PROPERTY Live` in the configuration:

```sh
cargo run -p dive-cli -- check \
  conformance/probes/native-temporal-operators/models/NativeHeadFails/NativeHeadFails.tla \
  --compat=tlc --lncheck=final --lasso=tlc --workers=4 --seed=0 --fp=0 --output=json
```

Exit 13 means a liveness violation. Schema 2 reports the actual counters,
requested/used workers, configured/expanded/derived property names, final-only
schedule and independently validated stuttering counterexample. Here `Head(x)`
stays false. Changing the initial sequence to `<<TRUE>>` makes this bounded model
succeed. A refusal or resource limit is not a proof of success.

The Rust API uses the same explicit final mode:

```rust
use std::{path::Path, sync::Arc};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(
        "conformance/probes/native-temporal-operators/models/NativeHeadFails/NativeHeadFails.tla",
    );
    let analysis = dive_sema::analyze_file(path, &dive_sema::Options::default())?;
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n")
        .into_config()
        .ok_or("invalid config")?;
    let options = dive_check::CheckOptions {
        workers: 4,
        ..dive_check::CheckOptions::tlc(0, 0)
    };
    let report = dive_api::tlc_temporal::check_final(
        &ir,
        &cfg,
        &options,
        dive_liveness::phase_graph::Limits::default(),
    )?;
    let dive_api::tlc_temporal::Outcome::Violation(violation) = report.outcome() else {
        return Err("expected liveness violation".into());
    };
    assert_eq!(violation.document().properties, ["Live"]);
    assert_eq!(
        violation.validation().validation,
        dive_trace::Validation::Valid
    );
    println!("{:?}", violation.document().properties);
    Ok(())
}
```

Run that exact Cargo-owned example from the repository root:

```sh
cargo run -p dive-api --example native_temporal_operator
```

The earlier standalone producer retains its original
[`user-example.rs`](../conformance/probes/native-temporal-operators/user-example.rs).
Inspect `Outcome` and its private producer-derived evidence; a caller-supplied
word or graph does not certify a model. See the [final API guide](tlc-final-api.md).
The independent word evaluator supports both profiles; this explicit final TLC
facade continues to refuse the default profile and exact temporal storage.

The unary native actuals accept exactly one ordinary expression argument at constant
or state level; [Append](native-sequence-operators.md) admits exactly two. The original call and captured scope stay intact for evaluation;
the implementation does not invent a native operator body. Every supplied
operator actual is inspected even when unused. Configured targets, redirected
or value-overridden native targets, and an explicitly supplied same configured
native target refuse conservatively because the current operator representation
does not retain configured origin. Builtin, unknown, other native, recursive,
higher-order, opaque and effectful operator targets remain outside this slice.

Before a native formal reads caller states or captured/cached values, a bounded
walk inspects stored representation, namespace and ordering without calling a
provider. Stored tuples intentionally use their implicit ascending integer
order; records, general functions and Enum/Raw sets must use the run's order.
Deferred sets stay refused even when cached. This additional runtime walk shares
the existing finite-binding work accounting, capped at 50,000 inspections and
128 levels; small caller limits can stop sooner. Source planning and independent
word proof keep their existing node/step/depth and finite-binding bounds. These
are structural bounds, not aggregate memory or evaluation-time guarantees.

`Cardinality(IF x THEN {1} ELSE {})` is supported for a boolean state variable.
The separate [stored-successor admission](stored-successor-material.md) now
accepts authoritative normalized Raw/Enum set states such as `x = {1}`.
The original native producer retains its three named refusals as historical
evidence; a fresh complete20-case regression capture validates the newly
admitted Cardinality cases at1/3/4/8 workers. Cached providers stay refused.
`Head(<<>>)` and `Len(TRUE)` retain their actual pinned runtime failures and
native unsupported outcomes; this increment earns no error-class agreement for
them. Wrong arity and an action argument rejected by SANY remain reference
semantic rejections, not runtime support claims. Host effects remain explicitly
gated even when TLC never calls an unused actual.

Evidence and source contracts are in the
[probe README](../conformance/probes/native-temporal-operators/README.md) and
[design notes](../conformance/probes/native-temporal-operators/DESIGN.md).
This feature adds supported bounded contracts, not full temporal-family or
ordinary scheduling certification. Historical corpus reports belong to their
original immutable producers.

A separate unchanged 140-model census of the preceding known-user forwarding
producer (`7683204`) records 53 agreements, 85 explicit refusals and two timeouts,
with harness exit 1. It adds seven agreements to the earlier material producer.
It is retained in the [complete census](../conformance/studies/temporal-operators-final-small/README.md).
Those outcomes do not describe this later native-actual producer; its evidence
is the complete 20-row control selection and all 80 width observations above.
