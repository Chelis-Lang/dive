---- MODULE Unsup ----
\* A temporal property with a VIEW: refused by the core, as by graph capture.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Live == <>(x = 2)
View == x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
====
