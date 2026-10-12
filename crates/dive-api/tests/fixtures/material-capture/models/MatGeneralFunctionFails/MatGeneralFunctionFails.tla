---- MODULE MatGeneralFunctionFails ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = FALSE
Next == \E v \in {[i \in {1,3} |-> i]} : v[3] = 3 /\ x' = x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == []<>(x = TRUE)
====
