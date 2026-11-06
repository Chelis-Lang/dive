---- MODULE ImportedRegisterHelperOrderedHolding ----
EXTENDS ReviewBase, Naturals, TLC
VARIABLE x
Hidden == TLCSet(0,0) /\ TLCSet(0,1)
Target == Hidden
A == INSTANCE ReviewPredicate
Init == x=0 /\ TLCSet(0,0)
Next == A!Pred /\ x'=TLCGet(0)
Inv == x \in 0..1
====
