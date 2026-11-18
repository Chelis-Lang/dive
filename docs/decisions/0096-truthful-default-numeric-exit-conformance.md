# 0096: Truthful default numeric-exit conformance

- **Status:** accepted
- **Date:** 2026-10-13

## Context

ADR 0010 and ADR 0086 require numeric process exits as observable results.
The native CLI and `dive-api::check::exit` already publish stable default
statuses (D-008): property violation 10 and rejected input 1, while the pinned
TLC uses 12 and 150 for the measured safety and frontend cases. Ordinary and
worker-specific model comparison omitted numeric exits. Historical small-tier
reports from `e8c82c0e039f8da6308796a5bdf09ec5ab79a4fe` therefore credited
14 default rows per width as Agree despite unequal physical exits. Four
existing semantic ExpectedDivergence rows also omitted their exit differences.
The diagnosis read all four complete reports, pinned goldens and their hashes,
and executed old/new comparator, mapper and real CLI controls. It found a
preexisting accounting defect, not a change to the native numeric contract.

Schema 1 preserves actual absent, zero or unrelated exits even when its deciding
class maps to safety. A blanket `fields = ["exit_code"]` exception would hide
those failures. Finite primary temporal EC2116 may legitimately exit 12;
classification must retain its existing stream-shape checks.

## Decision

Preserve the documented native runtime contract. Compare `OracleResult.exit_code`
exactly in ordinary and worker-specific model comparisons, including absent
codes, independently of count/worker leniency. Do not normalize process status,
replace the actual exit, or change producer or classifier semantics.

Extend the established expected-divergence registry with one optional typed
`exit_code_pair = { expected = N, actual = M }`. Codes are integers in 0..255
and must differ. Field globs cannot cover `exit_code`. The pair must occur as an
exact numeric diff, the observed verdict must match the entry, and all other
diffs must remain within its existing fields. Independent trace validation is
unexcusable. Native safety, action-property, liveness and deadlock verdicts
require a whole counterexample, including when a worker-specific comparison
omits witness shape. When verdicts match, a selected golden that carries an
error witness also retains that obligation even if the oracle's payload is
unavailable or omitted. A differing verdict cannot earn Agree; an approved
semantic Success result such as D-006's P2Choose_str needs no counterexample
merely because TLC's different verdict has one. This selects the actual verdict's
contract and does not waive independent validation of a required word.
This is a requirement for independent replay, not certification by oracle
metadata. Absent and metadata-only observed traces are unconfirmed
without consulting a claimed-valid callback; present JSON or text-step payloads
still require independent validation. Validation receives the same selected
primary or worker-specific golden as comparison. Ordinary success, assumption,
frontend and evaluation outcomes whose contract carries no witness remain
legitimate without one. A matching default result is ExpectedDivergence, never Agree;
the TLC and override profiles never use the registry.

D-023 registers only the 14 measured exact model paths and their pairs; the
four established D-010/D-004/D-006 entries retain their existing narrow fields
and add their measured exact pairs. No future default-liveness, scheduling,
count, property or other divergence is authorized by these entries.

## Consequences

For this exit-only historical correction, default 90 Agree / 4 ExpectedDivergence
/ 60 Unsupported becomes an estimate of 76 / 18 / 60 at each width, conditioned
on every other obligation. Original reports and floors remain visible. This
is neither a fresh current-main measurement nor approval of a new 76 floor.
Before acceptance, the lead required complete corrected parent/current
four-profile measurements, the full gate and independent review. Every
genuinely exact prior agreement remains protected. The at-least-95% eventual parity target and full
roadmap remain unchanged. No pins, goldens, corpus selections or resource bounds
change.

The completed correction in [PR #266](https://github.com/Chelis-Lang/dive/pull/266)
was published as `b2bc0a2919369178e7b10b91944bff38e9e64b57`, with the exact tree
of tested producer `643b0f1351feccca56968cd8032093f53ccd2c8d`. Fresh independent
review, the full local gate, all eight corrected parent/current measurements,
and required source and post-merge CI passed. Both widths retain TLC 94 Agree /
60 Unsupported and default 76 Agree /18 ExpectedDivergence /60 Unsupported,
with unchanged per-model outcomes and no disagreement, timeout, oracle error or
pending validation. This confirms the accounting estimate without authorizing
a silent new numeric floor. The delegated measurement repair preserves the
actual corrected parent agreement set; historical default-floor comparisons
remain failed. See [the scoreboard](../scoreboard.md#corpus-scoreboard)
and [D-023](../divergences.md#d-023-truthful-default-numeric-exit-accounting).
