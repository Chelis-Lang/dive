# Final-only temporal checking with TLC witness selection

Rust callers can explicitly run bounded final-only temporal checking through
`dive_api::tlc_temporal::check_final`. This API constructs and checks the model's
own graph, selects TLC's first deciding strongly connected component and greedy
counterexample, then independently rebinds and validates the recovered behavior
both in memory and from its JSON representation. It never trusts a caller's
`validated` flag or supplied graph as proof.

The CLI also exposes this same private certificate boundary explicitly:

```sh
cat > Final.tla <<'TLA'
---- MODULE Final ----
VARIABLE x
Init == x=0
Next == UNCHANGED x
Live == []<>(x=1)
====
TLA
cat > Final.cfg <<'CFG'
INIT Init
NEXT Next
PROPERTY Live
CFG
cargo run -p dive-cli --release -- check Final.tla --compat=tlc \
  --lncheck final --lasso tlc --seed 0 --fp 0 --workers 4 --output json
```

This example violates `Live`, so the command exits 13. A supported
safe model exits 0. Refusals and bounds exit 3, internal failures exit 4; unverified
binding errors exit 1 and make no TLC error-class claim. Existing safety stops keep
their actual safety class/exit and available prefix statistics.

This is a separate API from `dive_api::temporal::check_final`, which implements
the bounded short-witness mode. Ordinary strict `dive check` temporal parity and
TLC's periodic checking schedule are still pending. Selecting this API or the
explicit flag pair deliberately
selects **final-only** scheduling and **TLC** witness selection; it does not make
unimplemented schedules available through the global compatibility profile.

## Run a model

The workspace crates are currently unpublished. From a Rust caller with these
workspace dependencies:

```rust
use std::sync::Arc;
use dive_api::tlc_temporal::{self, Outcome};
use dive_check::CheckOptions;
use dive_liveness::phase_graph::Limits;

fn check() -> Result<(), Box<dyn std::error::Error>> {
    let source = "---- MODULE Final ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x=1)\n====\n";
    let analysis = dive_sema::analyze_source("Final.tla", source, &Default::default());
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let config = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n")
        .into_config().ok_or("invalid model configuration")?;
    let options = CheckOptions { workers: 4, ..CheckOptions::tlc(0, 0) };
    let report = tlc_temporal::check_final(&ir, &config, &options, Limits::default())?;
    assert_eq!(report.scheduling(), "final-only");
    assert_eq!(report.selection(), "tlc");
    match report.outcome() {
        Outcome::Violation(violation) => {
            let json = violation.document().to_json();
            println!("{json}");
        }
        Outcome::Safety(safety) => {
            println!("Safety-only: {}", safety.result().verdict.kind());
        }
        Outcome::Holds(evidence) => {
            println!("Checked {} states", evidence.execution.capture.stats.distinct);
        }
    }
    Ok(())
}
```

Use `CheckOptions::tlc(polynomial, seed)` and fingerprint storage. The selected
polynomial and seed are explicit; worker width does not change the supported
selected result. The receipt records actual requested/used workers, concrete
safety counters and the checked OOS/PEM groups separately from temporal work.
The output keeps original configured property names, such as `Live`, separate
from the backend's derived role names, such as `Identity`.

`Violation` is privately constructed. Its immutable `candidate()`, `document()`
and `validation()` getters expose the concrete periodic word, graph/component
selection evidence and the actual independent JSON replay report. `Holds` means
all supported final graph groups were checked without acceptance; it retains the
fingerprint store's collision qualification. It is not an exact-store guarantee.

## Supported scope and refusals

The implemented slice covers the bounded pure planning, tableau, ordered concrete
successor graph and final SCC/witness path, including selected fairness, multiple
OOS, scoped local argument, revisitation, state/action recurrence and safe models.
It is not a claim that every TLA+ construct or temporal model is supported.

The API returns a typed error instead of reporting success when it cannot certify
a stage. Current named boundaries include exact storage, the default enhanced
profile, VIEW, symmetry, ALIAS/recovery, accepting-sink shortcut reconstruction,
initial error replay, periodic/RL/postcondition/possible hooks, unsupported
planning shapes, and impure/introspective/runtime-name/opaque captures. Explicit
sequential requests and model-driven sequential fallback are also refused until
that observer path is certified. Fixed entry/cell/inspection and word-position
limits produce refusals; limits have not been raised. These bounds do not account
aggregate bytes or metadata.

Admission rejects refused source effects before startup binding creates a host.
It inspects genuine native operator authority, every user definition, possible
configuration replacement targets through their actual module-context aliases and standard-module top-level zero-arity
startup bodies. It does not evaluate a predicate to guess whether it is pure.
This conservative policy may refuse an unused user definition containing an
opaque operator capture. Pure local helpers remain supported. Existing shared
constructor/context ownership admission remains a separate audited boundary.

Inspect `Failure.error.code()` and the typed deciding error. `Failure.workers`
contains available actual width facts; early admission reports zero used workers,
and lost evidence stays absent. `Failure.evidence` retains completed graph facts
when available. Refused or invalid independent replay never becomes a certified
violation. These error codes do not claim complete TLC error-class equivalence.

## Evidence and remaining parity work

The complete [probe selection](../conformance/probes/tlc-final-check/README.md)
retains 30 pinned-reference cases, including 11 reference/helper failures that
receive no error-class agreement credit. Nineteen completed references match SCC
anchors, pop/visit order, greedy postfix, closure and prefix fields at 1/3/4/8
workers. Every supported recovered word is independently replayed; changed
initial states, Next/closing edges, loop targets and claims are rejected. An
independent acceptance/path census covers 4,096 small graphs.

The underlying observation copy preserves final roots and adjacency, not TLC's
original disk offsets or write history. Original unmodified `finalCheck` output
is retained separately. Old broad studies keep their recorded outcomes; these
focused controls do not add ordinary strict corpus agreement or finish temporal
parity. Full small-tier measurements, scheduled checking,
remaining refusal/error boundaries and wider resource coverage are next.

The exact design and algorithm provenance are in [ADR 0040](decisions/0040-pinned-final-scc-and-witness.md)
and [provenance](provenance.md). Feature families F-019–F-022 remain partial and
F-023 remains missing in the [parity ledger](tlc-parity.md).

## JSON and typed CLI API

The explicit mode uses check schema 2; ordinary checking retains schema 1.
`options` records `lncheck: "final"`, `lasso: "tlc"`, the TLC profile/seed/polynomial,
`temporal_scope: "bounded-final-only"`, and
`ordinary_temporal_schedule_supported: false`. `temporal_bounds` contains the
actual unchanged capture, formula, slot/value/cell/work, product and pool ceilings;
these are entry/work counts, not aggregate byte budgets.

Completed results include actual engine counts, collision-qualified storage,
requested/used workers and an `execution` completion receipt. A violation carries
all original configured `properties`, their joined TLC message name, and a
tagged independently replayed `counterexample.trace`. A self-loop is represented
as a stuttering ending, as in TLC's printed trace; other cycles carry a one-based
`back_to` marker. The document never substitutes backend role labels for original
configured property identities. The final-mode harness requires scalar joined
names, complete claim vectors, loop markers and serialized state rows to agree
with the actual document before fresh replay. These consistency checks and a
`validation` flag alone cannot certify a violation.

On failure, `execution.stage` names the deciding stage, `diagnostics` preserves
the stable deciding code/message, and statistics are present only when actual
prefix/completed graph evidence exists. A completed capture followed by a failed
SCC/replay stage is not a temporal result. Lost worker evidence is `null`; early
admission reports zero used workers. Unknown temporal error classes remain
unverified rather than receiving an inferred TLC classification.

Rust file callers select `CheckRequest { temporal: TemporalMode::FinalTlc, .. }`
with `compat_tlc: true`. `CheckReport::tlc_temporal_result()` exposes the immutable
private-certified report, `tlc_temporal_failure()` the typed failure, and `result()`
a safety stop. Neither a public JSON validation flag nor graph/count equality can
create a certificate.

## Conformance mode

The harness implementation `dive-final-tlc` is distinct from ordinary `dive` and
the observational final/short study:

```sh
cargo run -p dive-conformance --release -- run --impl dive-final-tlc \
  --impl-profile tlc --dive-bin target/release/dive --corpus examples \
  --tier small --workers 1 --no-sany --no-summary
```

It records the exact final/TLC flags, rejects per-feature mixed overrides, and
requires the golden's clean command to contain `-lncheck final`. It reads only the
explicit schema-2 final/TLC contract and performs another independent bind/replay;
the implementation's own validation flag earns no credit. The complete selection
retains every refusal, failure and timeout. Any agreements certify only the
supported final-only mode; they do not certify TLC's ordinary periodic schedule.

## Complete small-selection baseline

The first unchanged 140-model small Examples run on the frozen CLI source
produced **140 named refusals and no agreements**, with all 138 started
subprocesses retained. It earns no general corpus credit. The focused eight
positive models pass at 1, 3, 4 and 8 workers, but most corpus specifications
still encounter conservative scope/effect admission or
unimplemented witness recovery. Safety-only models currently refuse an empty
temporal OOS list; a future real safety route must preserve their actual
counters and independently validated witnesses. See
`conformance/studies/tlc-final-cli-first/README.md` for the complete historical
selection, raw streams, source hashes and explicit missing shell-exit fact.

Current shipping admission first uses the shared callback-free public IR
validator, then the shared bound-model context authority before planning and
before fresh independent replay. The preserved legacy source remains historical
evidence, not an alternative admission authority.

Explicit frontier/predecessor spill options currently refuse in this final mode:
its temporal capture certificate does not retain those storage receipts. Ordinary
safety spill remains a separate supported contract. The capture bounds report
the actual 65,536-successor source-unit ceiling and fixed maximum of eight
concurrent expanded source units independently of requested/actual pool width.
Malformed public IR preserves the shared `DIVE-EVAL-C001`–`C007` diagnostic
before option dispatch or host creation; bound-model metadata admission is
also checked before planning/replay. No opaque callback runs to guess validity.

## Safety-only route

The explicit `--compat=tlc --lncheck final --lasso tlc` mode also accepts a
supported model with **no implied temporal obligations**. That decision comes
from fresh source/configuration binding, not an empty property list or a JSON
route tag. Initial properties, invariants, implied action properties, state/action
constraints and deadlock checking are still ordinary safety obligations.

```sh
dive check conformance/probes/tlc-final-safety/models/SafetyHold/SafetyHold.tla --config conformance/probes/tlc-final-safety/models/SafetyHold/SafetyHold.cfg --compat=tlc --lncheck final --lasso tlc --workers 4 --seed 0 --fp 0 --output json
```

Rust callers receive `tlc_temporal::Outcome::Safety`, whose private receipt
retains the actual safety engine `CheckResult`. Inspect `result().verdict` before
claiming success. `Ok(report)` can carry a resource stop or named refusal.
`safety.limits()` returns the exact caller-provided `Limits`, retained privately by
the producer. `safety.validation()` and `safety.document()` expose independently replayed
finite counterexamples; they are absent when no witness was selected.

The CLI uses the final facade's unchanged default entry/work limits: 1,000,000
retained graph representatives, 8,000,000 retained generated/implicit-stutter edges, 32,000,000
generated candidates and 65,536 successors per source. Source waves remain
bounded at eight concurrently expanded sources, with at most 64 requested
workers. Rust callers can supply their own existing `Limits`; the safety receipt
retains those actual values rather than advertising CLI defaults. Temporal
phase-L/tableau/product limits are retained as supplied, but this route does not
consume them to construct a temporal product; shared pre-binding and worker
admission still apply. These are count ceilings, not aggregate heap/byte budgets. Conservative
source quotas can stop before global ceilings. Ordinary engine insertion precedes
graph admission: with a graph-state bound of three on the infinite counter,
the resource stop truthfully records four distinct states (the deciding inserted
state is counted). This does not complete capture or bound the entire seen-set
heap. Exceeding any ceiling is a real
resource outcome with actual stopping evidence, never completed exploration.

The schema-2 `execution` object names `stage: "safety"`,
`route: "bounded-safety-only"`, `temporal_obligations: 0`,
`temporal_certificate: false` and `product_constructed: false`.
`options.temporal_bounds` serializes the actual producer-retained limits,
including non-default Rust caller bounds; it never substitutes the CLI defaults.
`capture_completed` is true only when actual safety exploration finishes with
success. A supported finite violation carries its original finite ending and
requires fresh in-memory and lossless JSON replay against a separately bound
model. Refused or invalid replay cannot issue violation credit. The harness
independently derives the route from the actual model before comparing results;
ordinary schema-1 handling is unchanged. Initial invariant/property failures can
stop during inline initial-state enumeration, before the requested pool starts;
the report then truthfully records one used initialization worker, a sequential
initial stage and its actual reason. Later pool outcomes report the actual pool
width rather than inventing work at the requested width.

VIEW, symmetry, randomness, effectful/opaque startup/captured operators, host
hooks and transient disk storage remain explicitly staged refusals in this first
route. Implied initial properties combined with state constraints, and implied
action properties combined with state/action constraints, are conservatively
refused before pool work: independent replay does not yet reproduce the earlier
TLC stopping order on out-of-model states. No tableau, OOS, SCC or temporal
acceptance proof is inferred from a safety receipt.

The original 140-refusal small-model census is preserved unchanged. New native
safety controls and any subsequent full census identify their own source/binary
and retain every refusal, error and limit. They do not rewrite that historical
result or supply general liveness parity.

A subsequent unchanged 140-model census on the frozen safety-route source has
**5 full agreements and 135 named refusals**, with 138 actual subprocesses,
no disagreement or timeout, and actual harness exit 0. DieHard's seven-state
finite witness was independently validated. This observation is retained in
[the complete source-bound study](../conformance/studies/tlc-final-safety-current/full-small-study/README.md);
it does not certify temporal products, periodic scheduling or whole semantic
families. The failed launcher setup is separately preserved and executed zero
native checker subprocesses.

Pure state conditional predicates now use the corrected shared TLC-shaped
planner through final execution and fresh replay. See the
[conditional guide](tlc-state-if.md) for runnable CLI/Rust examples and exact
remaining restrictions. Earlier complete small-model reports remain unchanged
and source-qualified; this increment supplies selected controls, not new broad
corpus or family certification.

Direct temporal conditionals now use pinned shared-guard tags and exact numbered
particles through final execution and fresh replay. The
[temporal conditional guide](tlc-temporal-if.md) supplies runnable CLI/Rust
examples and complete selected controls. Upstream-refused temporal argument
forwarding into IF-bearing bodies and accepting-sink early witnesses remain
named unsupported boundaries, including unused forwarding calls in admission.
All existing bounds, effect/context checks and ordinary-schedule restrictions
remain in force.
