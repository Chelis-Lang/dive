---- MODULE ImportedRegisterConfigHelperOrdered ----
EXTENDS ReviewBase, Naturals, TLC
CONSTANT D
VARIABLE x
Hidden2 == TLCSet(0,0) /\ TLCSet(0,1)
Hidden == Hidden2
Target == D
A == INSTANCE ReviewPredicate
Init == x=0 /\ TLCSet(0,0)
Next == A!Pred /\ x'=TLCGet(0)
Inv == x=0
====
