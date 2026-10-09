---- MODULE LvNoFair ----
\* Without fairness, a behaviour may stutter forever. []<>(x = 0) is violated
\* by stuttering at x = 1 (2 states) or at x = 2 (3 states). Which
\* counterexample does TLC report, and how does it end?
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x
Prop == []<>(x = 0)
====
