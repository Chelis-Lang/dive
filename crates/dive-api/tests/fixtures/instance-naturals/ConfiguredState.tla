---- MODULE ConfiguredState ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
CONSTANT Op(_)
TimesX(n) == n*x
Read == Op(2)
Inv == Read = x+x
====
