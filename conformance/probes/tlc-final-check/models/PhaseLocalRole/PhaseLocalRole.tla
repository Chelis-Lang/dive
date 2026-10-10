---- MODULE PhaseLocalRole ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' \in {0,1,2}
Flip == x' \in {0,1,2}
Spec == Init /\ [][Next]_x
Identity(p) == p
Live == LET Alias(q) == Identity(q) IN Alias([]<>(x = 2))

====
