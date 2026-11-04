---- MODULE MultiDomains ----
EXTENDS Naturals, TLC
VARIABLE x,y
Init == /\ x \in {0,1} /\ y \in {0,1} /\ x=y
Next == /\ x'=x /\ y'=y
====
