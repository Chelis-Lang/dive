# Operator arguments in explicit final checking

Bounded final TLC checking supports pure user operators passed as formal operator
arguments. Their captured caller values, local LET parents and instance contexts
stay intact. This removes a native refusal; it does not change TLC semantics or
defaults. Ordinary periodic checking remains incomplete.

For example:

```tla
---- MODULE OperatorExample ----
VARIABLE x
Init == x = FALSE
Next == UNCHANGED x
Use(F(_)) == []<>F(x)
Capture(saved) == LET Local(v) == v = saved IN Use(Local)
Live == Capture(TRUE)
====
```

Use `INIT Init`, `NEXT Next` and `PROPERTY Live` in the configuration. The model
violates Live on the single FALSE state with a stuttering ending. The executable
fixture and Rust example are in conformance/probes/temporal-operators.

```sh
dive check conformance/probes/temporal-operators/models/OpCaptureFails/OpCaptureFails.tla --compat=tlc --lncheck final --lasso tlc --seed 0 --fp 0 --workers 4 --output json
```

The schema2 report retains actual counts, requested/used workers, final-only
schedule, TLC selection, original configured/expanded names and derived role
labels separately. Every violation is freshly rebound and independently checked
both in memory and as JSON; a named claim, route tag or reported validity alone
cannot certify it. A completed constant-only property can have no temporal
obligation; source-derived safety evidence is not temporal-product evidence.

For Rust callers:

```rust
use std::{path::Path, sync::Arc};
let file = Path::new("conformance/probes/temporal-operators/models/OpCaptureFails/OpCaptureFails.tla");
let analysis = dive_sema::analyze_file(file, &Default::default())?;
let ir = Arc::new(dive_ir::lower(&analysis)?);
let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n")
    .into_config().ok_or("invalid config")?;
let options = dive_check::CheckOptions { workers: 4, ..dive_check::CheckOptions::tlc(0, 0) };
let report = dive_api::tlc_temporal::check_final(&ir, &cfg, &options, Default::default())?;
if let dive_api::tlc_temporal::Outcome::Violation(v) = report.outcome() {
    assert_eq!(v.document().properties, vec!["Live"]);
    assert_eq!(v.validation().validation, dive_trace::Validation::Valid);
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

Known nonrecursive user targets with ordinary value formals are proved without
execution, even when supplied but unused. Captured lexical and substitution
contexts are projected under the same work budgets: each retained lazy expression
and known user operator body is independently checked with its original scope.
Hidden unused operator arguments cannot bypass this check after binding inlines
the outer call. Opaque lazy functions and unproved operator captures refuse. Pure constant/state/temporal actuals,
operator relay and supported nested local/INSTANCE contexts have paired pinned
controls. Exported *operator* formals in this selection execute in TLC; exported
*temporal expression* formals that TLC rejects2213 remain refused.

Native/builtin operator values, higher-order actual targets, recursive targets,
value overrides, unresolved/opaque callbacks and host/checker effects refuse.
Configured operator constants remain refused: their binding erases origin, so a
definition used as any configured operator target also refuses when passed
explicitly. Unknown analysis formals are never runtime values. Actual binder
domains are still checked at each state/edge under the existing finite ceilings.
The shared50k capture work/128 depth and existing formula/context/graph limits
remain structural budgets, not aggregate heap bounds. Same-owned stored material
also must match the run's ordering; [the material guide](material-capture.md)
explains the tuple exemption.

Admission failures report zero used workers and no completed evidence. Resource
stops and unsupported combinations cannot report temporal success. Only the
explicit bounded TLC final profile is exposed here; this is not full temporal
language, ordinary scheduling, higher-order callback or corpus certification.
See [the final API guide](tlc-final-api.md) and
[the source/evidence contract](../conformance/probes/temporal-operators/DESIGN.md).

Capture traversal keys distinguish the substitution head and cutoff as well as
the lexical frame. Owners remain retained until traversal ends, so allocation
addresses cannot be reused as proof identity. Dynamic `@` contexts conservatively
refuse; value hashing/equality is never capture authority.

Former blanket-refusal examples now have explicit acceptance controls. For
`T(p) == []<>p`, `H(F(_),p) == F(p)`, `P == H(T,x)`, a false initial state may
stutter forever; the pin and the bounded final checker both report the resulting
liveness violation. Acceptance does not imply that `P` holds. A recursive actual
remains refused before evaluation, and an actual definition whose own parameters
include an operator remains outside the analysis accessor's admitted subset.

The subsequent bounded [native actual slice](native-temporal-operators.md) admits
Len/Head/Cardinality separately. Earlier receipts below retain their original
known-user source identity; they are not recredited as native-source runs.
