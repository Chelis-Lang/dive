---- MODULE ErAssertInv ----
\* Assert fails in an invariant at x = 2: TLC 2132.
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == Assert(x # 2, <<"x is", x>>)
====
