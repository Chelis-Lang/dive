# 0093: Temporary validator parity exception

- **Status:** accepted, temporary
- **Date:** 2026-10-13

## Context

The rescue plan requires removal of trace replay admission proofs and retention
of the measured corpus agreement baseline. A direct independent audit found
that these requirements conflict for three existing cases: removing the proofs
leaves correct checker results with explicitly unsupported independent witness
validation. Restoring the old machinery or accepting an unchecked witness would
undermine the rescue's soundness and simplification goals.

At the first measured simplified candidate, the same 154-model TLC selection
has 91 agreements at one worker and 92 at four workers, versus 93 before the
cut. CheckpointCoordination improves to agreement; the three cases below lose
independent trace validation. There are zero disagreements. These measurements
do not establish the default-profile or final integrated candidate results.

The maintainer delegated the acceptance decision to the lead, with eventual
parity as the objective. This exception permits consolidation to proceed while
keeping the missing runtime support visible.

## Decision

Permit only agreement-to-pending-trace-validation regressions for these exact
corpus/model keys in the validator simplification change:

| Corpus | Model |
| --- | --- |
| examples | specifications/DieHard/MCDieHardest.cfg |
| probes | specifications/P5TLCGetSet/P5TLCGetSet.cfg |
| probes | specifications/P9Random/P9Random.cfg |

Each final profile and worker report must retain the complete original
selection and explain its actual per-model transitions. No other regression,
disagreement, timeout or oracle error is accepted. The original baseline and
its failed comparisons remain visible; this is not a new global count floor.
Goldens, bounds, harness outcomes, negative controls, independent-validator
requirements and the full local/CI gates are unchanged. A pending witness is
not agreement, and the final corpus goal remains at least 95% agreement.

Retain named refusals for uncheckable register access and random draws. Init
uses its correct startup control level without changing those guards. Do not
restore replay certificates, infer stream positions from a witnessed branch,
accept an arbitrary in-range random value, or inherit caller registers as
authority for startup effects.

## Follow-up and removal

Startup-register support needs a separately reviewed private runtime contract:
reconstruct binding/startup effects in order, isolate caller state, and account
for or refuse later/off-trace effects and other Init candidates. A startup
snapshot alone does not establish those obligations.

Random support needs a separately reviewed evaluator membership/runtime
contract. Preserve actual seeded positive controls and in-range seed-mismatch,
out-of-range, discarded/conditional/multiple-draw negative or refusal controls.
The validator must remain independent of the successor enumerator.

Track both obligations under issue #241. Remove this exception when the named
cases regain independently validated agreement in the required profiles and
worker settings without unrelated regressions. This decision grants no full
corpus, modes, liveness, performance or release completion credit.
