---- MODULE ErTrivialSpec ----
VARIABLE x
Init == x = 0
Next == x' = x
Never == FALSE
Spec == Init /\ Never /\ [][Next]_x
====
