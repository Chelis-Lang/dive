---- MODULE RowDifferentPrimary ----
EXTENDS Naturals
VARIABLES s, x
Init == s = 0 /\ x = 0
Pick == s' = 1 /\ x' = CHOOSE v \in {1, 2} : v > 5
Next == s'=1 /\ x'=1 \div (x-x)
Spec == Init /\ [][Next]_<<s, x>> /\ WF_<<s, x>>(Pick)
Live == <>(s = 1)
====
