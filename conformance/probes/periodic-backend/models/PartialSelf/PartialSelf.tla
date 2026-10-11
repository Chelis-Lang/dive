---- MODULE PartialSelf ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == UNCHANGED x
Live == []<>(x=1)
Spec == Init /\ [][Next]_x
====
