---- MODULE RepeatedCandidateSet ----
EXTENDS Naturals
VARIABLES x,y
Init == /\ x={0} /\ y=0
Next == /\ x'={0} /\ y' \in 0..2
====
