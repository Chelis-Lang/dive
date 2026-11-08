---- MODULE Deadlock ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
====
