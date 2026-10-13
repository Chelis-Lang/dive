# Temporal checking

Ordinary checking runs the supported temporal subset:

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterTemporal.cfg --seed 7 --workers 4 --output json
```

The default profile checks after completed BFS levels when distinct-state growth
since the last check exceeds 10%, and always checks at exhaustion. Under
`--compat=tlc`, ordinary checking uses the final schedule, matching the pinned
`-lncheck final` oracle. `--lncheck final` selects final checking in either
profile; `--lncheck deterministic` selects completed levels explicitly.
`--lasso short` is optional. `--lasso tlc` remains a usage error: TLC's arbitrary
lasso choice is not an observable parity requirement.

A candidate becomes a violation only after a fresh independent model accepts
both its typed word and serialized trace, including closing edge, fairness and
all false configured claims. An unexpanded frontier cannot establish disabledness:
ENABLED is evaluated exactly, independently of captured outgoing edges. A prefix
without a validated lasso continues searching. Holds requires exhaustive capture.
Models without temporal claims keep the ordinary safety path and one startup binding.

Temporal atoms use original generated targets, including seen-state occurrences.
Original Init aliases and source variants are checked too. If equal stored
identities produce different atom truth or incompatible Next/constraint rows,
checking returns a source-named `DIVE-L0038` refusal. Replay that cannot safely
preserve effects, names, composition or bounds also refuses at that point.
Actual evaluator errors retain their typed class/code; they cannot become Holds.
Bounded identity preparation uses the capture's actual exact/fingerprint options
and preserves constructor classes. It grants no host access or capture authority.

Temporal reports use schema 2; ordinary safety reports keep schema 1. Reports
include actual generated/distinct/depth/queue counters, requested/used workers,
seed, store/collision qualification and selected schedule. `execution.exhausted`
and `capture_completed` are false on a deliberate prefix stop, with
`completed_level` naming its boundary. Default deterministic stopping can prevent
a later safety failure and differ from final counts: this is the disclosed D-014
schedule difference, not an assertion that prefix counters equal final goldens.
A short lasso alone does not label TLC values as overrides; explicitly selecting
deterministic scheduling under compat does.

The example has no fairness forcing increments. Stuttering at zero is a valid
violation of eventual termination. The deterministic route stops at its first
completed level, while final checking explores the full finite graph. Both words
are independently validated. Temporal violations exit 10 by default and 13 under
compat. Planning/label evaluation errors retain their error class, TLC code and
exit; 2213/2214 formula classification runs before tableau construction. Bounds
and unsupported validation remain refusals, never success.

VIEW/SYMMETRY temporal lifting, fresh effects/register reads/random draws,
action-composition replay, unsupported scopes, spill/seen-disk/checkpoint,
postconditions with temporal checks and valid `_PERIODIC` hooks remain named
refusals. Invalid `_PERIODIC` names/arity retain TLC codes 2229/2228. Finite
independent ENABLED covers and every formula/capture/product bound still apply.
Original Init occurrences share the graph edge/occurrence bound. Additional
alias cells, replay and callback-free transport/expansion work consume existing
budgets; consumer work never changes raw generated/distinct counts.
Limits count entries/work, not total process memory; fixed capture waves retain
at most eight concurrently expanded source units.

Rust callers retain `temporal::check_final` and its exhausted-only evidence.
`temporal::check_with_schedule_with_evidence` also returns a distinct
`Outcome::PrefixViolation`, whose exploration is a Prefix, never Complete.
See [the API guide](../temporal-api.md) and [ADR 0094](../decisions/0094-ordinary-completed-level-liveness.md).
