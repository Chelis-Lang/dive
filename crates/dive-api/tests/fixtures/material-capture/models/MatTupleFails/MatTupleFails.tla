---- MODULE MatTupleFails ----
EXTENDS Naturals, TLC

VARIABLE x
Init == x = FALSE
Next == \E v \in {<<1, 2>>} : v[1] = 1 /\ x' = x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == []<>(x = TRUE)
====
