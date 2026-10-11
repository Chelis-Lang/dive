---- MODULE MatStringHolds ----
EXTENDS Naturals, TLC
CONSTANT Data
VARIABLE x
Init == x = FALSE /\ Data = "token"
Next == Data = "token" /\ x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == []<>(x = TRUE)
====
