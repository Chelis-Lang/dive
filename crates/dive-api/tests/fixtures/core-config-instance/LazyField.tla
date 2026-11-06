---- MODULE LazyField ----
EXTENDS Naturals, TLC
CONSTANT C(_)
VARIABLE x
F == [k \in STRING |-> TLCSet(0,0) /\ TLCSet(0,1)]
Hidden == F.a
Target(n) == Hidden
Init == x=0 /\ TLCSet(0,0)
Next == C(0) /\ x'=TLCGet(0)
Inv == x=0
====
