---- MODULE Stack ----
EXTENDS Naturals
VARIABLE x
RECURSIVE F(_)
F(n) == IF n = 0 THEN 0 ELSE 1 + F(n - 1)
Init == x = F(200000)
Next == UNCHANGED x
====
