---- MODULE PartialMultiple ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x' \in {0,1,2}
Live == <>(x=1) /\ <>(x=2)
Spec == Init /\ [][Next]_x
====
