---- MODULE MatNestedFails ----
EXTENDS Naturals, TLC
CONSTANT a, b
VARIABLE x
Init == x = FALSE
Next == \E v \in {<<[tag |-> "ok", vals |-> {a, b}], <<1, 2>>>>} : v[1].tag = "ok" /\ x' = x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == []<>(x = TRUE)
====
