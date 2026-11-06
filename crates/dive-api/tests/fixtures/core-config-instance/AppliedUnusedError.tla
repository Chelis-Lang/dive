---- MODULE AppliedUnusedError ----
EXTENDS Integers
CONSTANT C(_)
VARIABLE x
Target(n)==TRUE
Init==x=0
Next==C(1 \div 0) /\ x'=x
Inv==x=0
====
