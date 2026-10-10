---- MODULE LdSubsetHolds ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Values == SUBSET BOOLEAN
Live == \A s \in Values : []<>(x = (TRUE \in s))
====
