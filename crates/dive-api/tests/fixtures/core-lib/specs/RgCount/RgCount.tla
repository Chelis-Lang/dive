---- MODULE RgCount ----
\* TLCSet and TLCGet on the core: integer and named registers that the
\* ASSUMEs set, updated and read in TLC's order of expansion, printed, and
\* read in an invariant.
EXTENDS Naturals, TLC
VARIABLE x
ASSUME TLCSet(1, 0) /\ TLCSet("s:n", "start")
Init == x \in {0, 1}
Next == \/ /\ x < 4
           /\ TLCSet(1, TLCGet(1) + 1)
           /\ PrintT(<<x, TLCGet(1), TLCGet("s:n")>>)
           /\ TLCSet("s:n", x)
           /\ x' = x + 1
        \/ /\ x = 4
           /\ UNCHANGED x
Inv == TLCGet(1) < 100
====
