---- MODULE DefinedFunction ----
EXTENDS Integers
VARIABLES es,step
F == [i \in 1..2 |-> 1 \div 0]
Init == /\ es={} /\ step=0
Next == /\ step<2 /\ F \notin es /\ UNCHANGED es /\ step'=step+1
====
