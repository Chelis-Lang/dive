---- MODULE MinChoosePlain ----
EXTENDS Naturals
VARIABLES s, x
Init == s = 0 /\ x = 0
Pick == s' = 1 /\ x' = CHOOSE v \in {1, 2} : v > 5
Next == Pick
Spec == Init /\ [][Next]_<<s, x>>
Live == <>(s = 1)
====
