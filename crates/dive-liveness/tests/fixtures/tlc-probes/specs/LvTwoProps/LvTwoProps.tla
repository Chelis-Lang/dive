---- MODULE LvTwoProps ----
\* Two properties, both violated: PROPERTIES P1 P2. Which does TLC report,
\* and does it report both?
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P1 == <>(x = 7)
P2 == <>(x = 8)
====
