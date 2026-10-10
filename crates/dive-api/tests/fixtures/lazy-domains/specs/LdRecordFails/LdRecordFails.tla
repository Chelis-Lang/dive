---- MODULE LdRecordFails ----
VARIABLE x
Init == x = FALSE
Next == UNCHANGED x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Values == [a : BOOLEAN, b : BOOLEAN]
Live == \A r \in Values : []<>(x = r.a)
====
