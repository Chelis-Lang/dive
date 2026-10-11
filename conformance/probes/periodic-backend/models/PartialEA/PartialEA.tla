---- MODULE PartialEA ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x' \in {0,1}
Live == <>[](x=1)
Spec == Init /\ [][Next]_x
====
