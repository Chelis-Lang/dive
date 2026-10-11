# Fixed tuple projection replay

Issue #215 extends sealed whole-word tuple-history proofs through fixed tuple
aliases and scoped parameters. Original alias/conditional observations retain
producer `f8b3b51521ba166dd5ce5f8031a8bfb7185acefb` in `preimplementation/`.
The conditional tuple can swap a tuple variable with a string and therefore
cannot prove each variable unchanged. Its original unassigned enumeration error
is separate from the corrected assigned clean-primary violation.

Focused development controls cover fixed nested tuples, lazy parameter scopes,
LET, forged successors the conditional permutation, wrong lazy parameter binding and the depth-64 refusal. An actual faulty proof
that intersects variable occurrence across conditional branches incorrectly
admits the word; the negative catches it. Original malformed fixture spacing is
retained before the corrected 20-test pass. Full current-source runtime and gate
qualification remain pending; no feature-family certificate is claimed.
