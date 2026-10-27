---- MODULE WideRepeat ----
\* A level wider than the parallel engine's chunk (4,096 states per worker):
\* x % 5000 repeats a state across chunks. No violation.
EXTENDS Naturals
VARIABLES x, y
Init == x \in 0..19999 /\ y = 0
Next == y = 0 /\ y' = 1 /\ (x' = x % 5000 \/ x' = x + 20000)
Inv == x # 39000
====
