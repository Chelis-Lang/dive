---- MODULE LdFunctionHolds ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Values == [{1,2} -> BOOLEAN]
Live == \A f \in Values : []<>(x = f[1])
====
