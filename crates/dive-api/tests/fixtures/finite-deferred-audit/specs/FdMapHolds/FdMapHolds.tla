---- MODULE FdMapHolds ----
EXTENDS Naturals, Sequences, FiniteSets
VARIABLE x
Domain == {f[2] : f \in {g \in [1..2 -> BOOLEAN] : g[1] = FALSE}}
Init == x = FALSE
Next == x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == \A b \in Domain : []<>(x = b)
====
