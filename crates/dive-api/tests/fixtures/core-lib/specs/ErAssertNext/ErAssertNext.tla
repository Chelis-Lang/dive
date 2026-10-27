---- MODULE ErAssertNext ----
\* Assert fails in the next-state relation from x = 2: TLC 2132.
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == /\ x < 4
        /\ Assert(x # 2, "x reached 2")
        /\ x' = x + 1
====
