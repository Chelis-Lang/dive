---- MODULE FdMapFails ----
EXTENDS Naturals, Sequences, FiniteSets
VARIABLE x
Domain == {f[2] : f \in {g \in [1..2 -> BOOLEAN] : g[1] = FALSE}}
Init == x = TRUE
Next == UNCHANGED x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == \A b \in Domain : []<>(x = b)
====
