---- MODULE DirectState ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
Double == 2*x
Inv == Double = x+x
====
