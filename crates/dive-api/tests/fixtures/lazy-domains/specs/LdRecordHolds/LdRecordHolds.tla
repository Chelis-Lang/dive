---- MODULE LdRecordHolds ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Values == [a : BOOLEAN, b : BOOLEAN]
Live == \A r \in Values : []<>(x = r.a)
====
