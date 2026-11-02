---- MODULE GenCase ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == CASE x = 0 -> x' = 1 [] OTHER -> x' = 2
====
