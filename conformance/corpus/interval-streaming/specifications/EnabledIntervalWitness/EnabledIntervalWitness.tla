---- MODULE EnabledIntervalWitness ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == UNCHANGED x
Inv == ENABLED (x' \in 1..1000000000 /\ IF x'=1 THEN TRUE ELSE 1 \div 0=0)
====
