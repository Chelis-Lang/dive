---- MODULE RndDeadlock ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Next == x' = RandomElement({1, 2}) /\ x' = 2
Safe == x >= 0
====
