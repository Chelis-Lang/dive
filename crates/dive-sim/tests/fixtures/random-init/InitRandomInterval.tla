---- MODULE InitRandomInterval ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = RandomElement(1..1000000)
Next == x' \in {0,2,4} \/ x'=x+1
Safe == x>=0
====
