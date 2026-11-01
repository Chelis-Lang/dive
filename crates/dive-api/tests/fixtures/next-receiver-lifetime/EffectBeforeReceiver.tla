---- MODULE EffectBeforeReceiver ----
EXTENDS Integers,TLC
VARIABLES es,step
Init == /\ es={} /\ step=0
Next == /\ PrintT("actual-before-receiver") /\ [i \in 1..2 |-> 1 \div 0] \notin es /\ UNCHANGED es /\ step'=step+1
====
