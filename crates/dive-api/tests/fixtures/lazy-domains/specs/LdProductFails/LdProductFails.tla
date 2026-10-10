---- MODULE LdProductFails ----
VARIABLE x
Init == x = FALSE
Next == UNCHANGED x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Values == BOOLEAN \X BOOLEAN
Live == \A <<a,b>> \in Values : []<>(x = a)
====
