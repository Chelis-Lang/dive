---- MODULE BindingUnprovedCall ----
EXTENDS Naturals
CONSTANTS C, Op(_)
VARIABLE x
RECURSIVE F(_)
F(n) == IF n=0 THEN TRUE ELSE F(n-1)
Target == Op(1)
Init == x=0
Next == C /\ x'=x
Inv == x=0
====
