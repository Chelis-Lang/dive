---- MODULE MidLevelViolation ----
\* Probe: what TLC counts when an invariant fails part-way through a level.
\* State 0 has successors 1, 2, 3 (action A) and 10 (action B). The invariant
\* fails at 2, the second successor of the first action. If TLC stops at the
\* violating successor, it has generated 3 states (the initial state, 1 and
\* 2), found 3 distinct states, and left 1 state (1) on the queue; 3 and 10
\* are never generated.
EXTENDS Naturals
VARIABLE x
Init == x = 0
A == x' \in {x + 1, x + 2, x + 3}
B == x' = x + 10
Next == A \/ B
Inv == x # 2
====
