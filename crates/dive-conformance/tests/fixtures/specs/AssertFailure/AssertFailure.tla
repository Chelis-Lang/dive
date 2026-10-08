---- MODULE AssertFailure ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == /\ Assert(x < 2, "x got too big")
        /\ x' = x + 1
====
