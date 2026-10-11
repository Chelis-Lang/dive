---- MODULE MatEnumSetFails ----
EXTENDS Naturals, TLC
CONSTANT Data
VARIABLE x
Init == x = FALSE /\ 1 \in Data
Next == 1 \in Data /\ x' = x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == []<>(x = TRUE)
====
