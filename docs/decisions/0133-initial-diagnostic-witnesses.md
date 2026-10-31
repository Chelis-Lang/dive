# 0133: Initial diagnostic witnesses outside formal counterexamples

- **Status:** accepted
- **Date:** 2026-10-31

## Context

The pinned `comparison-receiver-primary` controls distinguish an actual initial
invariant comparison failure from TLC's diagnostic replay of that failure.
Eight classified initial controls decide with fingerprint exception 2147,
initial report 2102 and physical exit 75. Their generated oracle results carry
`trace: null`: TLC's recorded initial diagnostic state is not a formal behavior
counterexample. The Init-generation control also has no formal counterexample.

The first implementation reproduced the class but emitted an anonymous
`GENERAL` counterexample. Fresh independent validation rejected it. Replacing
that EC with 2147 would discard the underlying semantic obligation; ADR 0118
requires a named invariant-error word to retain its original cause. Merely
accepting a named cause word would still not prove the separate diagnostic
receiver lifetime or deciding wrapper.

Schema 3 already separates incomplete-Init `error_witness` evidence from formal
counterexamples. That field is closed to its partial-terminal obligation. Its
meaning must not be widened to cover a complete initial diagnostic state.

## Decision

1. Preserve the formal initial counterexample's absence, including in the public
   report. Add optional schema-3 `diagnostic_witness` for the separately owned
   initial diagnostic source. Keep existing `error_witness`, `counterexample`,
   native versions and ordinary trace comparison rules unchanged.
2. The auxiliary payload uses the existing native version-2
   `invariant-error` ending, with the configured invariant name and original
   underlying EC. It does not claim that 2147 is the underlying evaluation
   cause. No new ending, payload version, result-changing default or dependency
   edge is introduced.
3. Accept the new initial 2147/2102 relationship only after independently
   consuming both obligations: the exact named cause word, and a fresh
   session's actual normal and diagnostic Init/invariant execution. The latter
   must establish the actual typed receiver method, constructor source and
   diagnostic lifetime. A producer verification flag, matching header, ordinary
   valid word or serialized source annotation supplies no such authority.
4. This route is bounded to the first actual Init emission and first configured
   invariant, without VIEW, symmetry, constraints, effects or unproved state
   set/provider history. Cached, stored, foreign-owner and later-row lifetimes
   receive no inferred certificate. Owned interruption, resource and bug
   transports retain their actual identities.
5. Require the auxiliary word for this newly admitted route even when the
   oracle's formal trace is absent. Missing, mutated, unproved or refused
   diagnostic evidence cannot earn agreement. The normalized formal trace stays
   absent; exits, classes, counts and depth retain their ordinary comparisons.

## Consequences

The report can preserve TLC's observable absence of a formal counterexample
while retaining independently checkable diagnostic evidence. Existing version-2
payload meanings remain stable. Consumers that need this classification must
consume the new auxiliary field and both proof obligations; decoding it alone
does not establish the wrapper.

Focused controls retain old reports and partial Init witnesses, original cause
and invariant names, first-emission and lifetime boundaries, actual owned stops,
and mandatory evidence checks. Context-only 2103 streams remain unclassified;
other Init/Next methods and broader error contracts receive no new credit.
