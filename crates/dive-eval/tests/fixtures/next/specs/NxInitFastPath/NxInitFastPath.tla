---- MODULE NxInitFastPath ----
\* Probe: once every variable of an initial state is assigned, the rest of
\* the initial predicate is evaluated as booleans: 2 initial states, not 4
\* or 6.
VARIABLE x
Init == x \in {1, 2} /\ (TRUE \/ TRUE) /\ \E i \in {1, 2, 3} : TRUE
Next == UNCHANGED x
====
