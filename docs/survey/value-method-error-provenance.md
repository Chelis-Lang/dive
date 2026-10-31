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
