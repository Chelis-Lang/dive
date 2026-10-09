---- MODULE LvLeadsTo ----
\* (x = 1) ~> (x = 3): from 1 the system may return to 0 forever instead of
\* reaching 3. Violated by a lasso through x = 1.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == \/ x = 0 /\ x' = 1
        \/ x = 1 /\ x' \in {0, 3}
        \/ x = 3 /\ x' = 3
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == (x = 1) ~> (x = 3)
====
