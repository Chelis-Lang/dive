---- MODULE LvEventuallyAlways ----
\* x may oscillate between 0 and 1 forever, or settle at 2. <>[](x = 2) is
\* violated by the oscillating lasso.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == \/ x = 0 /\ x' = 1
        \/ x = 1 /\ x' \in {0, 2}
        \/ x = 2 /\ x' = 2
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == <>[](x = 2)
====
