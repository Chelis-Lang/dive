---- MODULE MultipleInitial ----
EXTENDS Integers
VARIABLES es,step
Init == /\ es={} /\ step \in 0..1
Next == /\ step<2 /\ [i \in 1..2 |-> 1 \div 0] \notin es /\ UNCHANGED es /\ step'=step+1
====
