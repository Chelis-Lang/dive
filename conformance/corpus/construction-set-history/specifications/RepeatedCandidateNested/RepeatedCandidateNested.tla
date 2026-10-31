---- MODULE RepeatedCandidateNested ----
EXTENDS Naturals
VARIABLES x,y
Init == /\ x=[i \in 1..1 |-> {0}] /\ y=0
Next == /\ x'=[x EXCEPT ![1]={0}] /\ y' \in 0..2
====
