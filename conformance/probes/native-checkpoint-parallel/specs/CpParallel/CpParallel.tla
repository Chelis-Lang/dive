---- MODULE CpParallel ----
EXTENDS Naturals
VARIABLE x
Init == x \in 0..3
A == x < 12 /\ x' = x + 4
B == x < 12 /\ x' = x + 5
Next == A \/ B
====
