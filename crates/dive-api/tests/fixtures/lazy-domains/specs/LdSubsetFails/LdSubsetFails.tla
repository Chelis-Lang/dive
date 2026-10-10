---- MODULE LdSubsetFails ----
VARIABLE x
Init == x = FALSE
Next == UNCHANGED x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Values == SUBSET BOOLEAN
Live == \A s \in Values : []<>(x = (TRUE \in s))
====
