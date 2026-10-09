---- MODULE LvAlwaysEventually ----
\* x may stay in the 0/1 cycle forever or visit 2. []<>(x = 2) is violated by
\* the 0/1 lasso.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == \/ x = 0 /\ x' = 1
        \/ x = 1 /\ x' \in {0, 2}
        \/ x = 2 /\ x' = 0
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == []<>(x = 2)
====
