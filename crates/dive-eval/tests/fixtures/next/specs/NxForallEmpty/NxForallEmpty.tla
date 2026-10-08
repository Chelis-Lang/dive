---- MODULE NxForallEmpty ----
\* Probe: \A over an empty set is the continuation alone.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x < 2 /\ (\A i \in {} : x' = i) /\ x' = x + 1
====
