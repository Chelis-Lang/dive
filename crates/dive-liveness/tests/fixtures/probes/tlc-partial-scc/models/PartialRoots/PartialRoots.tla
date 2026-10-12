---- MODULE PartialRoots ----
EXTENDS Naturals
VARIABLE x
Init == x \in {0,1}
Next == x' = IF x<2 THEN x+1 ELSE x
Live == []<>(x=2)
Spec == Init /\ [][Next]_x
====
