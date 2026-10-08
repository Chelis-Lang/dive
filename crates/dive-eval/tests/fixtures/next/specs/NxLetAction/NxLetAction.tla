---- MODULE NxLetAction ----
\* Probe: a LET definition used as a disjunct is enumerated each time it is
\* used: A \/ A generates two successors.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x < 2 /\ LET A == x' = x + 1 IN A \/ A
====
