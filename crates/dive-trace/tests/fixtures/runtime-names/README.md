# Runtime-name trace control (#124)

`ReplayNames` completes safely on the clean pinned oracle (seed 0, fp 0,
one worker): 3 generated, 2 distinct states, depth 2. There is no dumpTrace
pass. The raw stdout/stderr and source/config hashes are retained in the
oracle directory. These are regression fixtures, not edited harness goldens.

`bad.json` claims a transition from `s = "unset"` to `s = "7"` and an
invariant violation. The old independent validator accepted it: decoding the
witness interned `"7"` before the predicate created `"6"`, so the witness
changed CHOOSE's encounter order. `validate-trace.json` preserves that false
positive; `check.json` preserves the safe native search before the repair.

TLC-order validation now refuses runtime-name models explicitly, including
lossy TLC interpretation and the direct decoded-trace API. A witness prefix
cannot establish strings created by off-trace states. Seeds and fingerprints
do not provide that history. Canonical default validation still accepts the
actual `"6"` behavior and rejects the forged `"7"` transition.
