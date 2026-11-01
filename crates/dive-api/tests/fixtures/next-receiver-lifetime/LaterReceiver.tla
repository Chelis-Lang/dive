---- MODULE LaterReceiver ----
EXTENDS Integers
VARIABLES es,step
Init == /\ es={} /\ step=0
Next == IF step=0 THEN /\ UNCHANGED es /\ step'=1 ELSE /\ [i \in 1..2 |-> 1 \div 0] \notin es /\ UNCHANGED es /\ step'=step+1
====
