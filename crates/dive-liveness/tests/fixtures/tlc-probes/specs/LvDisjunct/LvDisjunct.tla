---- MODULE LvDisjunct ----
\* PROPERTY [](x < 2) \/ <>(x = 9) cannot be split into safety and liveness
\* parts, so all of it goes to the liveness checker. Every behaviour violates
\* it (x reaches 2 and never 9). How does TLC report a violation whose safety
\* part fails on a finite prefix?
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == [](x < 2) \/ <>(x = 9)
====
