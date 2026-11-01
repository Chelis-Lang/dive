---- MODULE EmptyLazyQuery ----
EXTENDS Integers
VARIABLES es,step
Init == /\ es={} /\ step=0
Next == /\ step<2 /\ [i \in 1..2 |-> 1 \div 0] \notin es /\ UNCHANGED es /\ step'=step+1
====
