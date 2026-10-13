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

TLC-order validation now refuses a verdict whenever decoding or checking the
trace created a name after the model was bound (ADR 0016, amendment 3). Here
the witness's `"7"` and the predicate's `ToString(x)` both create names, so
the word is refused. Canonical default validation still accepts the actual
`"6"` behavior and rejects the forged `"7"` transition.
