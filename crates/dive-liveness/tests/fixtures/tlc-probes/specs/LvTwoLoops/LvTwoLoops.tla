---- MODULE LvTwoLoops ----
\* Two violating lassos. From 0, x moves to 1 or to 10 (1 is enumerated
\* first). 1, 2, 3, 4 is a cycle of four states; 10, 11 a cycle of two. Both
\* violate <>(x = 99). Which lasso does TLC report: the first found, the
\* shortest, or another?
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == \/ x = 0 /\ x' \in {1, 10}
        \/ x \in 1..3 /\ x' = x + 1
        \/ x = 4 /\ x' = 1
        \/ x = 10 /\ x' = 11
        \/ x = 11 /\ x' = 10
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == <>(x = 99)
====
