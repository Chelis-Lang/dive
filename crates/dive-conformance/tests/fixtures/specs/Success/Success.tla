---- MODULE Success ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 4
TypeOK == x \in 0..3
====
