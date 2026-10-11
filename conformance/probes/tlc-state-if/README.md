# Pure state-IF final admission (#168)

The complete 15-row primary selection invokes the actual pinned
`Liveness.processLiveness`, `LiveCheck.addInitState`, `addNextState`, graph slots,
SCC inspection and unmodified `finalCheck`. Eleven rows complete their reference
projection; four fail with deciding codes 2179 or 2250. Reference errors remain
selected and grant no invented native error-class agreement.

`IfStateHold` legitimately violates because a behavior may stutter at a false
state. `IfHoldTrue` is the separate tautological positive. `IfTableau` retains its
original temporal IF parsing; `IfEntry` explicitly parenthesizes the pure state
conditional before implication, testing entry particle consistency. No prior
observation is replaced by a new source or relabelled as a hold.

Delegating `Bin` wrappers preserve each predicate's result/exception and record
actual call order, including tableau particles. Pinned algorithms are unchanged.
Java oracle SHA256: `7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d`.

Original 12/13/14-row captures, the duplicate-field helper failure and all 48
pre-admission native outcomes are retained separately. Seven original integrated
source matrices and exact compressed source bytes precede the new current
matrix. Source checks do not retag earlier runs as current observations.

Reproduce with the existing Java17/jar flags on `capture.py --reason REASON` in
an isolated cache/checkout, preserving an earlier capture first. Run
`python3 -I scripts/check-tlc-state-if.py` to verify all selections/raw hashes.
Native execution/replay controls live in `crates/dive-api/tests/tlc_state_if.rs`.
The [user guide](../../../docs/tlc-state-if.md) gives runnable CLI/Rust examples,
fixed limits and remaining schedule/profile/identity/error restrictions.

The separate `native-current/` census retains all 60 selected CLI outcomes at
1/3/4/8 workers: 36 independently validated violations, four holds, and 20 named
refusals (12 before workers, eight after runtime work). All four deciding reference-error models remain uncredited refusals.
`IfTableau` also remains refused because the exact preserved initial particle
identity differs (pin 3/native 4); see [#170](https://github.com/Chelis-Lang/dive/issues/170).
Exact graph/particle/bin and independent mutation controls are asserted by the
API tests; matching words or counts cannot excuse an identity disagreement.

`user-example.rs` is extracted from the user guide. `user-example-controls/`
retains its successful compilation/execution receipts (prints `["Live"]`), while
`historical-user-example-before-config-fix/` retains the earlier Option-vs-Result
compilation error. `capture-native.py --binary PATH --out FRESH_DIRECTORY`
reproduces the native census and verifies source/binary identity throughout.
