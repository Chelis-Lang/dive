---- MODULE TmRegisters ----
EXTENDS Naturals, TLC
VARIABLE x
ASSUME TLCSet(1, 0) /\ TLCSet("s:n", "start")
Init == x \in {0, 1}
Next == /\ x < 4
        /\ TLCSet(1, TLCGet(1) + 1)
        /\ PrintT(<<x, TLCGet(1), TLCGet("s:n")>>)
        /\ TLCSet("s:n", x)
        /\ x' = x + 1
Inv == TLCGet(1) < 5
====
