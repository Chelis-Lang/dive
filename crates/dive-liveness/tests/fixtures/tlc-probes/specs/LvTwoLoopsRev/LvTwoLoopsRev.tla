---- MODULE LvTwoLoopsRev ----
\* LvTwoLoops with the roles swapped: the short cycle (1, 2) is enumerated
\* first and the long one (10 to 13) second. Together with LvTwoLoops this
\* shows whether TLC's choice follows exploration order or cycle length.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == \/ x = 0 /\ x' \in {1, 10}
        \/ x = 1 /\ x' = 2
        \/ x = 2 /\ x' = 1
        \/ x \in 10..12 /\ x' = x + 1
        \/ x = 13 /\ x' = 10
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == <>(x = 99)
====
