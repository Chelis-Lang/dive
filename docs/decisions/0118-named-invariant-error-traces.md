# 0118: Named invariant-error traces

- **Status:** accepted
- **Date:** 2026-10-28

## Context

Pinned TLC error fixtures `ErAssertInv`, `ErBagCardNonBag` and `ErBagCupNonBag`
stop with `TLC_INVARIANT_EVALUATION_FAILED` (2111), physical exit 76 and a finite
counterexample. Their underlying causes are respectively 2132, 2176 and 2169.
Both dive engines preserve these classes, counts and words, but formerly emitted
an anonymous underlying error/assertion ending. The harness compared that cause
to the deciding wrapper and rejected the claim. Independent validation then
re-raised the invariant error as an invalid word (#490). Retagging the cause as
2111 would discard the actual semantic obligation; accepting arbitrary final
errors would admit forged claims and earlier failures.

ADR 0108 freezes existing native trace tags and meanings. A new closed ending
requires a new native payload version and advance notice to known consumers.
The outer operation report schema is independent of the native trace payload.

## Decision

For invariant-site errors decided by 2111, retain the configured invariant name
through classification and emit native `dive-trace` version 2 with ending
`{"kind":"invariant-error","name":"Inv","ec":"TLC_VALUE_ASSERT_FAILED"}`.
The `ec` is the underlying evaluation cause, never the deciding wrapper. State,
value, label and length encodings retain their existing meanings. The outer
check/validation report remains schema 3. Initial-state, constraint,
action-property and successor-expansion error families are unchanged.

Existing endings continue writing native version 1. Readers retain legacy
version-1 decoding and accept version 2. A version-1 `invariant-error`, unknown
version or unknown ending is rejected. There is no fallback to a generic ending
when an older reader cannot interpret the claim. Known consumers receive advance
notice and update their readers before advancing their immutable producer pin;
landing this wire change is conditional on that notice.

The harness admits the wrapper/claim distinction only for this named ending and
2111. Independent validation must prove the finite word, require earlier states
and earlier configured invariants to pass, and reproduce the exact claimed cause
at the named final invariant. A non-Boolean invariant independently establishes
`TLC_EXPECTED_VALUE`; true and false Booleans establish no error. Resource,
interruption, unsupported and internal outcomes retain their existing refusal or
exception channels. Later checks do not run after the deciding invariant error.
No constraint exception is added (#499).

## Consequences

Correct invariant-error witnesses can receive independent validation without
weakening the anonymous error-ending boundary. New readers preserve old pinned
payloads; old readers fail closed on the new payload. Consumer readers must
handle native versions 1 and 2 separately from report schema 3, understand the
new named obligation and retain unknown-version/tag rejection. They must never
infer validity merely from the wrapper or successful JSON decoding.

Paired producer/validator controls cover both engines, profiles and workers 1
and 4 against the pinned fixtures. Negative controls cover wrong cause/name,
missing error, an earlier failing state or invariant, and false Boolean results.
Source mutations that remove cause equality or earlier-invariant checking must
fail. The remaining error-ending families under #490 are not closed by this
change.

## Qualified native replay context (2026-10-29)

`ErInvReplayError` retains an original named invariant cause (InvA, 2179),
first report 2111 and a different error during diagnostic replay (deciding
2103, exit 75). Its word has the same named invariant-error obligation, so it
uses the existing version-2 encoding. The producer uses the typed invariant
site, first report and deciding class to retain that original claim. The strict
harness does not admit the 2103 report distinction: native context metadata and
a valid InvA word alone cannot establish the secondary replay. No error ending
gains a wrapper exemption or new agreement credit.

Independent replay still proves the original invariant name and cause, with
earlier checks passing. It supplies no proof of diagnostic replay class or
counts; ordinary result comparison retains those obligations. The canonical
classifier leaves this pinned mixed-error stream unclassified. That outcome
is unchanged and earns no pinned-TLC agreement credit. No new ending, payload
version, report schema or consumer migration follows from this producer repair.
