---- MODULE FdFilterHolds ----
EXTENDS Naturals, Sequences, FiniteSets
VARIABLE x
Domain == {f \in [1..2 -> BOOLEAN] : f[1] = FALSE}
Init == x = FALSE
Next == x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == \A f \in Domain : []<>(x = f[1])
====
