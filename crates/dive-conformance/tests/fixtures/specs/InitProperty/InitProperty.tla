---- MODULE InitProperty ----
EXTENDS Naturals
VARIABLE x
Init == x \in {0, 1}
Next == UNCHANGED x
Spec == Init /\ [][Next]_x
StartsAtZero == x = 0
====
