---- MODULE ConfiguredEffect ----
EXTENDS Naturals, TLC
CONSTANT C
VARIABLE x
Hidden == TLCSet(0, 0) /\ TLCSet(0, 1)
Target == Hidden
Init == x = 0 /\ TLCSet(0, 0)
Next == (C /\ x' = 0) \cdot (x' = x)
Inv == x = 0
====
