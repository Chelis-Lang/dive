# 0093: Temporary validator parity exception

- **Status:** fulfilled; temporary acceptance exception retired
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
treat an in-range value as proof of seeded producer reproduction, or inherit
caller registers as authority for startup effects.

## Follow-up and removal

Startup-register recovery is tracked in [issue #256](https://github.com/Chelis-Lang/dive/issues/256),
and independent random membership in [issue #257](https://github.com/Chelis-Lang/dive/issues/257).

Startup-register support needs a separately reviewed private runtime contract:
reconstruct binding/startup effects in order, isolate caller state, and account
for or refuse later/off-trace effects and other Init candidates. A startup
snapshot alone does not establish those obligations.

Random support uses ADR 0016 amendment 5's narrow direct initial relation
contract. Preserve actual pinned positive controls, genuine out-of-domain and
false-ending negatives, and discarded/conditional/multiple-draw refusals.
The corrected #257 contract replaces the old in-range seed-mismatch negative:
P9 x=777 belongs to 1..1000000 and falsifies Inv, while the historical rejection
at Init compared it with the seed 0 producer sample 730968. That retained
measurement is evidence of the old reproduction contract, not an invalid
relation witness. Membership does not prove seeded-run reproduction.
The validator must remain independent of the successor enumerator.

Track both obligations under issue #241. Remove this exception when the named
cases regain independently validated agreement in the required profiles and
worker settings without unrelated regressions. This decision grants no full
corpus, modes, liveness, performance or release completion credit.

## Fulfilment (2026-10-13)

Private startup recovery merged through [PR #262](https://github.com/Chelis-Lang/dive/pull/262),
and direct initial random membership through [PR #263](https://github.com/Chelis-Lang/dive/pull/263).
The latter's tested source `e8c82c0e039f8da6308796a5bdf09ec5ab79a4fe` has the same
tree as merged main `9996b958e90eb793efb37f7f7d3ab6c1d8ab5746`. Fresh independent
review, the full local gate (1,599 tests, no skips, 456 SANY agrees), all four
complete unchanged 154-model runs and all three required source and main CI
checks passed.

All three named cases agree in every required profile and worker setting.
TLC workers 1/4 have 94/94 agrees; default workers 1/4 have 90/90, with four
expected divergences each. There is no pending validation, disagreement,
timeout, oracle error or per-model agreement loss against the current or original
baseline. The original TLC 93/default 90 floors pass. Issues #256 and #257 were
closed by hand after verification.

The temporary acceptance exception is no longer needed. Its original decision
and failed measurements above remain historical. Supported startup and random
relations keep the narrow boundaries in ADR 0016 and the trace guide; this
fulfilment grants no general validator, liveness, full corpus, modes,
performance or release completion credit.

### Numeric-exit qualification (2026-10-13)

The preserved default90/floor-pass observation above used a comparator that
omitted physical numeric exits. [ADR 0096](0096-truthful-default-numeric-exit-conformance.md)
records the bounded correction: 14 of those default Agree credits were false,
and four old ExpectedDivergence rows omitted exit differences. The 76/18/60
exit-only historical estimate is not a fresh certificate or approved new floor.
The validator recovery remains real; complete corrected conformance and no-loss
require fresh measurements and independent review. Original reports and floors
remain preserved.
