---- MODULE AppliedUsedError ----
EXTENDS Integers
CONSTANT C(_)
VARIABLE x
Target(n)==n=n
Init==x=0
Next==x'=x
Inv==C(1 \div 0)
====
