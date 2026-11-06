---- MODULE BoolHelperAsValue ----
EXTENDS Naturals, TLC
CONSTANT C(_)
VARIABLE x
Init == x=0 /\ TLCSet(0,0)
Hidden == PrintT("bool-helper")
Target(n) == IF Hidden THEN TRUE ELSE FALSE
Next == C(0) /\ x'=IF x=0 THEN 1 ELSE x
Inv == x \in 0..1
====
