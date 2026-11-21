---- MODULE Un ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x + 1 \/ x' = x + 2
Inv == x < 3
Al == x * 10
====
