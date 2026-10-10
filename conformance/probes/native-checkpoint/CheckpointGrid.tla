---- MODULE CheckpointGrid ----
EXTENDS Naturals
CONSTANT N
VARIABLES x, y
Init == x = 0 /\ y = 0
Next == \/ x < N /\ x' = x + 1 /\ UNCHANGED y
        \/ y < N /\ y' = y + 1 /\ UNCHANGED x
Inv == x <= N /\ y <= N
====
