# Initial-predicate level control

`Audit.tla`/`Audit.cfg` are the minimal startup-level control. The pinned
`tlc-2026.10.06.014338` oracle (upstream commit
`94d0c5070852b37b801bb8998c436383ab7ee37b`) checks one worker with seed/fp 0.
It reports invariant `Inv` at states `x = 0, 1, 2`, with 3 generated, 3 distinct
and depth 3, exit 12. `primary.json` compacts that primary result; tests consume
its states in plain and TLC lossy documents. The raw capture is regenerated
outside the repository, never committed here.

Invoke the manifest-pinned JRE and jar, with this pair copied into an external
scratch directory, as `tlc2.TLC -workers 1 -seed 0 -fp 0 -metadir states
-dumpTrace json trace -config Audit.cfg Audit.tla`. Compact `counterexample.state`
from `trace` and retain the oracle outcome separately. The full trace's labels
are unnecessary for these concrete predicate controls.

Init and ASSUME see level 0. Generated-state checks start at level 1. The test
adds bounded state/action/implied-property controls and rejects altered Init,
steps and endings; it also checks caller-host restoration after success,
invalid input and refusal. These controls do not certify register replay or
random stream positions.
