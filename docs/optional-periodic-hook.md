# Optional configuration-owned periodic diagnostics

A configuration without `_PERIODIC` now remains genuinely hook-free in the bounded
Rust diagnostic. `periodic_snapshot::bind_optional` and
`periodic_candidate::check_optional` derive absence from the original configuration;
you cannot supply TRUE, a predicate, an absence flag, graph or Valid result.
The existing `bind`/`check` APIs still require an explicitly configured hook.

This is an internal prerequisite for ordinary scheduling, not ordinary temporal
checking. An admitted witness describes a concrete violating behavior after two
fresh independent replays. An absent witness, successful safety EOF or partial
pass never means temporal Holds or model completion. Ordinary CLI periodic checking
and checkpoint composition remain unsupported. The exposed diagnostic model still
refuses ordinary capture/checking even when there is no configured hook.

## Run the Cargo example

```sh
cargo run -p dive-api --example optional_periodic_candidate --locked
```

Its original configuration has no `_PERIODIC`. It reaches genuine safety EOF,
privately selects a word, and freshly validates it in memory and through JSON.
Expected output: `origin=eof memory=Valid json=Valid model_proof=false`.

```rust
//! Admit only an actual producer-selected word; never an ordinary model proof.
use dive_api::periodic_candidate;
use dive_check::{CheckOptions, progress};
use dive_liveness::{periodic_lifecycle, phase_graph};
use std::{sync::Arc, time::Duration};
fn main() -> Result<(), String> {
    let source = "---- MODULE CandidateGuide ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=IF x=0 THEN 1 ELSE 1\nLive == <>(x=2)\n====\n";
    let analysis =
        dive_sema::analyze_source("CandidateGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .ok_or("configuration")?;
    let report = periodic_candidate::check_optional(
        ir,
        cfg,
        CheckOptions {
            workers: 4,
            ..CheckOptions::tlc(0, 0)
        },
        phase_graph::Limits::default(),
        periodic_lifecycle::Options::default(),
        progress::Options {
            interval: Duration::from_secs(3600),
            force: false,
        },
    )
    .map_err(|e| e.to_string())?;
    if report.run().configured_name().is_some()
        || report.run().definition().is_some()
        || report.run().expression().is_some()
    {
        return Err("original configuration must remain hook-free".into());
    }
    let witness = report.witness().ok_or("no admitted word")?;
    if !matches!(witness.origin(), periodic_candidate::Origin::Eof(_)) || report.model_proof() {
        return Err("EOF witness qualification".into());
    }
    println!(
        "origin=eof memory={:?} json={:?} model_proof={}",
        witness.memory_validation().validation,
        witness.json_validation().validation,
        report.model_proof()
    );
    Ok(())
}
```

## Presence, absence and stopping evidence

The optional report's `configured_name`, `definition` and `expression` all return
None for actual absence. Present roles retain configured spelling separately from
the selected replacement definition/body. A declared but unused `Hook == FALSE`
or `Hook(v) == v` does not create a role. False hooks still stop after liveness;
integer/evaluation errors preserve their original diagnostic disposition.

`progress::Options::force` publishes an initial progress request. It is separate
from `periodic_lifecycle::Options::force`, which forces liveness checking. Without
an actual hook or forced liveness, the controller may skip suspension/resume under
its runtime-ratio policy. No resume receipt is manufactured. Controller `Complete` can mean an early skip;
it is not evidence that suspension occurred, nor a temporal verdict. Request and EOF
origins remain distinct; only successfully published graph sources and physically
appended prefixes supply deciding-word authority. Controlled RequestPrefix tests
use an explicit test-only scheduler, separate from natural clock observations.

Only matching TLC evaluator/profile options execute. Default-profile requests
refuse before clocks/workers. Source/context, hidden unused startup effects,
configured aliases, recursion and opaque targets remain guarded before binding.
Present hooks retain the bounded pure zero-state grammar; state/action/temporal
references refuse. Existing startup/capture inspection policies (50,000 units,
128 depth), graph/source/Session and independent replay limits are unchanged.
These are structural/policy bounds, not aggregate memory or evaluator complexity
guarantees. Bounds, invalid ownership and replay failures never construct a witness.

The exact [26-row reference](../conformance/probes/optional-periodic-hook/primary/README.md)
observes real selected Java methods, not complete TLCWorker/model scheduling.
Its queue helper and post-return cleanup are explicitly observer actions. Actual
ClassCastException for a constant role is retained, not invented as a TLC code.
Startup effects that TLC permits remain conservative native refusals without
parity credit. See [ADR0074](decisions/0074-optional-configured-periodic-hook.md),
[mandatory snapshot contract](configured-periodic-snapshot.md) and
[candidate replay contract](producer-owned-temporal-candidate.md).

A failed initial division has distinct reference surfaces: direct `doInit` throws
2179, while full model checking reports GENERAL1000 containing2179 and deciding
nested-expression2103/exit75. The native diagnostic retains that actual deciding
class with requested width4/used1 before the pool, and starts no request lane.
Both [raw surfaces](../conformance/probes/optional-periodic-hook/README.md) remain
separate; a method exception is not substituted for a run's deciding class.
