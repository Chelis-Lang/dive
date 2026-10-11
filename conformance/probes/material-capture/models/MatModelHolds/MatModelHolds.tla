---- MODULE MatModelHolds ----
EXTENDS Naturals, TLC
CONSTANT Data
VARIABLE x
Init == x = FALSE /\ Data = Data
Next == Data = Data /\ x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == []<>(x = TRUE)
====
