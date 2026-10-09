---- MODULE M2Undefined ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x + step
Inv == x < 5
====
