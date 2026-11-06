---- MODULE DirectRegisterConfigHelperOrderedHolding ----
EXTENDS Naturals, TLC
CONSTANT C
CONSTANT D
VARIABLE x
Hidden2 == TLCSet(0,0) /\ TLCSet(0,1)
Hidden == Hidden2
Target == D
Init == x=0 /\ TLCSet(0,0)
Next == C /\ x'=TLCGet(0)
Inv == x \in 0..1
====
