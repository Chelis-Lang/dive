# Stored state values in final TLC checking

Explicit bounded final TLC checking accepts concrete Enum and Raw sets, including
already normalized Raw sets, nested inside tuples, records and general functions.
Strings and model atoms must belong to the bound model's namespace; supplied sets,
records and general functions must use the run's storage order. Same-owned atoms
alone do not establish ordering. Tuples have an implicit ascending integer domain
and retain their storage-order exemption.

For example, this model has a genuine stuttering liveness violation:

```tla
---- MODULE StoredSetFails ----
EXTENDS FiniteSets
VARIABLE x
Init == x = {1}
Next == UNCHANGED x
Live == []<>(~(Cardinality(x) = 1))
====
```

With `INIT Init`, `NEXT Next`, and `PROPERTY Live` in the configuration:

```sh
cargo run -p dive-cli -- check \
  conformance/probes/stored-successor-material/models/StoredSetFails/StoredSetFails.tla \
  --compat=tlc --lncheck=final --lasso=tlc --workers=4 --seed=0 --fp=0 --output=json
```

Exit 13 reports a liveness violation. Schema 2 includes actual counters, requested
and used workers, configured/expanded/derived property names, final-only schedule
and an independently validated counterexample. Changing `Live` to
`[]<>(Cardinality(x) = 1)` gives a bounded completed success. ResourceLimit,
unsupported and incomplete results never prove success.

The Rust API binds source afresh and accepts only private producer outcomes:

```rust,ignore
let analysis = dive_sema::analyze_file(&path, &Default::default())?;
let ir = std::sync::Arc::new(dive_ir::lower(&analysis)?);
let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n")
    .into_config().ok_or("invalid config")?;
let options = dive_check::CheckOptions {
    workers: 4,
    ..dive_check::CheckOptions::tlc(0, 0)
};
let report = dive_api::tlc_temporal::check_final(&ir, &cfg, &options, Default::default())?;
```

The complete runnable [Rust example](../conformance/probes/stored-successor-material/user-example.rs)
asserts the independently validated violation. See the [final API guide](tlc-final-api.md)
for exact mode, ceiling and proof qualifications. This explicit TLC facade still
refuses the default profile, temporal exact storage and ordinary periodic scheduling.

Before any fingerprint, comparison or predicate evaluation, phase-L inspects only
`stored_elements()` and stored function entries. Deferred/provider sets stay
unsupported even if their element cache has been materialized. Admission never
uses provider normalization, equality or hashing as purity authority. Constructing
or hashing an outer value can invoke a provider before any admission stage exists;
provider sentinel tests record that constructor baseline and require admission to
add zero callbacks. Invalid state namespaces or storage order return DIVE-L0041;
opaque successor sets return DIVE-L0042. A malformed bound model still refuses
through the shared context authority first.

The existing Session ceiling remains 8,000,000 cumulative material-value visits
and depth 256; each function-domain key and child counts. Caller-supplied limits
are retained. The context/native-operator walk's 50,000/depth 128 is separate and
unchanged: it is not an extra global limit on stored successor states. These are
work/retention ceilings, not aggregate byte or memory guarantees.

The [paired controls](../conformance/probes/stored-successor-material/README.md)
retain seven clean pinned 0/13 pairs and all 56 original named native refusals.
Fresh source-bound results must remain separate from historical censuses and
ordinary-schedule/family certification. Independent periodic words and fresh
memory/JSON replay cover closing edges and altered property/initial/ending controls.
