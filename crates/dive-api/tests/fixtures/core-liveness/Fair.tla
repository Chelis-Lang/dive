---- MODULE Fair ----
\* Weak fairness makes P hold; Q fails (y may stay 0).
EXTENDS Naturals
VARIABLES x, y
Init == x = 0 /\ y = 0
A == x < 3 /\ x' = x + 1 /\ y' = y
B == y' = 1 - y /\ x' = x
Spec == Init /\ [][A \/ B]_<<x, y>> /\ WF_<<x, y>>(A)
P == <>(x = 3)
Q == []<>(y = 1)
====
