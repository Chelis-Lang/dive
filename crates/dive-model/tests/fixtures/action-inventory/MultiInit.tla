---- MODULE MultiInit ----
EXTENDS Naturals
VARIABLE x
I == x = 0
J == x = 1
Init == I \/ J
Next == x' = x
====
