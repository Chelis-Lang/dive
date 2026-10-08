---- MODULE NxEnabled ----
\* Probe: ENABLED in an invariant, through TLC's enabled search.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Step == x < 3 /\ x' \in {x + 1}
Next == Step \/ (x = 3 /\ x' = 3)
Inv == ENABLED Step
====
