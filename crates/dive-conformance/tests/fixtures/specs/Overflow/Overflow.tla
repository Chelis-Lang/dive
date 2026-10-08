---- MODULE Overflow ----
EXTENDS Integers
VARIABLE x
Init == x = 1
Next == x' = x * 1000
====
