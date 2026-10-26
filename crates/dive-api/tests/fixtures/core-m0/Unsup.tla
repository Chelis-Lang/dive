---- MODULE Unsup ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Live == <>(x = 2)
Spec == Init /\ [][Next]_x /\ WF_x(Next)
====
