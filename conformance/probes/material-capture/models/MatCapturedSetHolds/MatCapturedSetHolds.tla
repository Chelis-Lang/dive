---- MODULE MatCapturedSetHolds ----
EXTENDS Naturals, TLC

VARIABLE x
Init == x = FALSE
Next == \E v \in {{1, 2}} : 1 \in v /\ x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == []<>(x = TRUE)
====
