---- MODULE MatFunctionHolds ----
EXTENDS Naturals, TLC

VARIABLE x
Init == x = FALSE
Next == \E v \in {[i \in {1, 2} |-> i]} : v[1] = 1 /\ x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == []<>(x = TRUE)
====
