---- MODULE LvBigCycle ----
\* A larger lasso search: a ring of N = 200 states with a chord from N \div 2
\* back to 0, so the strongly connected component is large. <>(x = N + 1) is
\* violated. How long is TLC's counterexample, and is its cycle the shortest
\* available?
EXTENDS Naturals
CONSTANT N
VARIABLE x
Init == x = 0
Next == \/ x' = (x + 1) % N
        \/ x = N \div 2 /\ x' = 0
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == <>(x = N + 1)
====
