---- MODULE ImportedRegisterImmediateHolding ----
EXTENDS ReviewBase, Naturals, TLC
VARIABLE x
Target == TLCSet(0,0) /\ TLCSet(0,1)
A == INSTANCE ReviewPredicate
Init == x=0 /\ TLCSet(0,0)
Next == A!Pred /\ x'=TLCGet(0)
Inv == x \in 0..1
====
