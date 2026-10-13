---- MODULE ModeMutation ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == \/ /\ x = 0 /\ x' \in {1, 2}
        \/ /\ x = 2 /\ x' = 3
        \/ /\ x = 1 /\ x' \in {1, 2, 4}
        \/ /\ x = 4 /\ x' \in {1, 2}
Safe == x # 3
====
