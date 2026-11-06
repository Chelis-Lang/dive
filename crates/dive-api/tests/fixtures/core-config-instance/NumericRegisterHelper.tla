---- MODULE NumericRegisterHelper ----
EXTENDS Naturals, TLC
CONSTANT C(_)
VARIABLE x
Init == x=0 /\ TLCSet(0,0)
Hidden == IF TLCSet(0,0) /\ TLCSet(0,1) THEN 5 ELSE 0
Target(n) == Hidden=5
Next == C(0) /\ x'=TLCGet(0)
Inv == x=0
====
