---- MODULE Walk ----
\* A random walk; the invariant fails only on traces that reach 14.
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == x' = x + 1 \/ x' = x - 1
Spec == Init /\ [][Next]_x
NotFourteen == x # 14
====
