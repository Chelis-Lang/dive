---- MODULE PartialGrowthSafe ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x<2 /\ x' = x+1
Live == <>(x=2)
Spec == Init /\ [][Next]_x /\ WF_x(Next)
====
