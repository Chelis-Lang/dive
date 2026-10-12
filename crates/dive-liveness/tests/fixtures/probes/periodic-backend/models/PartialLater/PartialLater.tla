---- MODULE PartialLater ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x' = IF x<2 THEN x+1 ELSE x
Live == []<>(x<2)
Spec == Init /\ [][Next]_x
====
