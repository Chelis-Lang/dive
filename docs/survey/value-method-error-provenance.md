# Value-method diagnostic replay

Issue #672 has a minimal control in
`conformance/corpus/constant-retry-lazy/specifications/WholeFunctionError`.
The named partial function can be applied at zero, but its whole-function
equality forces the failing application at one. The pinned TLC reports
GENERAL1000 with an underlying division2179, then decides
TLC_FINGERPRINT_EXCEPTION2147 with physical exit75, one generated and distinct
state, depth1, and one queued state. The reference previously claimed verified
TLC_NESTED_EXPRESSION2103 for this control.

The source mechanism is receiver construction during diagnostic replay.
`Tool.java` attaches a source to the new `FcnLambdaValue` before calling its
`toFcnRcd` method; the ordinary tool ignores that hook, while `CallStackTool`
records it. `FcnLambdaValue.toFcnRcd` catches the body failure and wraps it only
when that actual receiver has a source. The underlying division code does not
establish the deciding wrapper. A lambda returned by `CallStackTool.eval` is
copied to rebind its tool; that copy does not inherit the original source.
Stored and precomputed receivers therefore require their own lifetime proof.

The #620 equality extension follows the same hook for actual `TupleValue.equals`
and `RecordValue.equals` catches at the pinned source. Receiver construction is
packed with the existing function order/literal metadata; the measured 64-bit
layout stays Value16/Fcn8/FcnInner64 bytes. A process-unique owner domain seals
constructor ids, so an error in another evaluator cannot recover a foreign
receiver's source. Exhausting that packed namespace declines provenance rather
than reusing an identity. There is no per-receiver side table or extra allocation.

Only actual fresh tuple/record constructors are stamped. Cached constants,
shared aliases and stored Next values are not upgraded by diagnostic reuse.
Actual Init-generator replay is limited to the first emitted, history-free row
and first invariant without constraints, VIEW or symmetry; reconstructed row
equality only rejects mismatches and is not source proof. The existing controlled
closed-function inspector proves absence of state set/provider history, not
receiver provenance. Other rows remain unverified. A pure initial-generation
error before any emission can similarly repeat that actual generator.
Existing armed host control remains installed. Unproved effects or lifetimes
remain unverified, and native comparison refusals are not relaxed.

The `comparison-receiver-primary` corpus has twelve generated deciding TLC
controls and twelve SANY inputs. Nine selected initial constructor/rebuild
controls match2147/75 on the reference and Auto routes under TLC comparisons;
default D-009 successes stay unchanged. The later invariant and direct Next
integer errors retain their prior primary classes. The dynamic Next comparison
is still unverified: its pinned2147 needs a separate diagnostic-lifetime path.
Seven context-only streams produced no golden and remain separate negative
fixtures, including precomputed and stored-Next receivers. These selected
controls add no historical corpus or whole-engine parity credit.
