---- MODULE CandidateBeforeReceiver ----
EXTENDS Integers
VARIABLES es,step
Init == /\ es={} /\ step=0
Next == (/\ UNCHANGED es /\ step'=1) \/ (/\ [i \in 1..2 |-> 1 \div 0] \notin es /\ UNCHANGED es /\ step'=step+1)
====
