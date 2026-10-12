---- MODULE FdCaptureFails ----
EXTENDS Naturals, Sequences, FiniteSets
VARIABLE x
Domain == {f \in [1..2 -> BOOLEAN] : f[1] = x}
Init == x = TRUE
Next == UNCHANGED x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == []<>(\A f \in Domain : f[1] = FALSE)
====
