---- MODULE MatRecordHolds ----
EXTENDS Naturals, TLC

VARIABLE x
Init == x = FALSE
Next == \E v \in {[a |-> 1, b |-> 2]} : v.a = 1 /\ x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == []<>(x = TRUE)
====
