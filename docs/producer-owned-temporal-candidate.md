# Producer-owned temporal candidate admission

`dive_api::periodic_candidate::check` constructs its own configuration snapshot,
actual request service and deciding word. You supply original IR/configuration and
options/limits; you cannot supply a graph, candidate, witness, Valid flag or clock.
Only two fresh independent validations create the privately constructible witness.
This bounded Rust API implements issue #213; focused controls are passing and the
complete delivery gate is pending. Its exact focused source passes seven public
controls, two request/private selection units, one pre-dedup audit unit, the Cargo
guide and focused clippy. Earlier failed sources remain separate. Ordinary CLI checking is still unsupported for these schedules.

## Run the Cargo example

```sh
cargo run -p dive-api --example periodic_candidate --locked
```

Expected output after its selected genuine safety EOF:

```text
origin=eof memory=Valid json=Valid model_proof=false
```

```rust
//! Admit only an actual producer-selected word; never an ordinary model proof.
use dive_api::periodic_candidate;
use dive_check::{CheckOptions, progress};
use dive_liveness::{periodic_lifecycle, phase_graph};
use std::{sync::Arc, time::Duration};
fn main() -> Result<(), String> {
    let source = "---- MODULE CandidateGuide ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=IF x=0 THEN 1 ELSE 1\nLive == <>(x=2)\nHook == TRUE\n====\n";
    let analysis =
        dive_sema::analyze_source("CandidateGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .ok_or("configuration")?;
    let report = periodic_candidate::check(
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

## Outcomes and attribution

`Report::run()` retains actual requested/used workers, prefix counters, graph
publication, frontier append, request service/hook/error and EOF facts. An admitted
word retains original request versus EOF origin, original OOS/PEM indices and exact
product selection fields before pinned contiguous-fingerprint packing. Its property
claims are the original configured properties, separately from expanded action
origins and derived role labels. Both replay reports are inspectable. No outcome
claims model completion, temporal absence, ordinary schedule/count equality or
queue-suspension equivalence. An absent witness never means Holds. The exposed
snapshot model still refuses ordinary `_PERIODIC` checking.

Fresh model A decodes/re-homes the original lossless private word into its own name
table and checks it in memory. Separately rebound model B validates serialized,
parsed JSON. Neither replay trusts backend predicate bins. Invalid, Unsupported,
bounds, malformed words and missing source attribution return noncertifying failures
with the actual run retained. Error codes DIVE-API-P001 through P008 distinguish
binding, input, reconstruction, resource, attribution, validation, document and
thread failures; these are not ordinary TLC deciding error-class parity claims.

## Current scope and limits

TLC-profile matching evaluator/checker options, pure configured hooks and existing
Phase-L admission are required. Host effects, checkpoints, opaque/cached providers,
foreign contexts/names, opposite storage order, unproved forwarding and VIEW/symmetry
remain refused. Requests come only from the internal clock/claim lane. A candidate
before the hook prevents that hook/resume; false hooks, earlier safety errors and
quota stops remain diagnostics and do not become admitted words.

Only actual completed source callbacks contribute concrete representatives. The
separate candidate-source endpoint audits raw admitted originals before SetOfStates
deduplication/recursive writes. Different concrete values sharing a fingerprint,
unknown/unwritten sources and missing representatives refuse. Legacy service resource
policy is unchanged. Audit entries use the caller's existing capture-state ceiling;
material visits/depth/work remain within the unchanged Session quotas. Source metadata,
SCC/selection and reconstruction share graph work accounting. Retained entries and
inspection policy do not bound aggregate bytes, heap allocation, evaluator instructions
or constructed-value comparison complexity. Reconstruction/attribution have explicit
position/atom-cell/inspection refusals and independent validation retains its own
finite-binding and word bounds. No limits are raised to admit a witness.

Controlled request tests use the existing explicit dev-only returned-output rendezvous
and selected +Infinity ratio policy; the default library/release graph excludes that
seam. Natural timing outcomes remain separate. Clean pinned final controls establish
model semantics only, not asynchronous ordinary scheduling. The preserved first
`<>(x=1)` expectation failed because initial stuttering is legal without fairness;
its raw exit13 remains alongside a separate holding `<>(x=0)` control.

See [ADR 0070](decisions/0070-producer-owned-candidate-admission.md),
[request service](request-owned-periodic-service.md),
[temporal user guide](user/temporal.md), and the
[probe selection](../conformance/probes/producer-owned-candidate/README.md).

## Preserved development evidence

The first source-retention draft inspected the Observer field after paused service
had taken its state, so the actual candidate lacked original representatives. That
failed source/run is retained; the corrected source passes the genuine borrowed
state into the shared source callback. The original `s=<<"literal",0>>` test input
is rejected by both native syntax and pinned TLC (exit150). Spaced string, tuple,
record and set inputs each yield pinned exit13 and fresh native Valid memory/JSON
replay. The lexical failure never counts as a model-checking agreement.

Strict controls separately corrupt Init, Next, claim and ending shape, reject an
unwritten or missing selected record, exhaust remaining graph work and inject a
synthetic fingerprint ambiguity to test admission. That injection does not claim
an observed collision of the actual FP64 polynomial. A low capture ceiling
retains actual incomplete execution and no witness. These diagnostic controls do
not broaden ordinary schedule support or certify the feature family.
