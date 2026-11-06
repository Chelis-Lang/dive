---- MODULE BoolRegisterHelperAsValueHolding ----
EXTENDS Naturals, TLC
CONSTANT C(_)
VARIABLE x
Init == x=0 /\ TLCSet(0,0)
Hidden == TLCSet(0,0) /\ TLCSet(0,1)
Target(n) == IF Hidden THEN TRUE ELSE FALSE
Next == C(0) /\ x'=TLCGet(0)
Inv == x \in 0..1
====
