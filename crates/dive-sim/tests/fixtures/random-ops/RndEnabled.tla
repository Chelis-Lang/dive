---- MODULE RndEnabled ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Pick == x' = RandomElement({1, 2}) /\ x' = 2
Next == x' = x + 1 /\ ENABLED Pick
Safe == x >= 0
====
