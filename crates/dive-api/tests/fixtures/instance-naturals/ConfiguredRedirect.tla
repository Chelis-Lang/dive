---- MODULE ConfiguredRedirect ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
TimesX(n) == 0
Target(n) == n*x
Read == TimesX(2)
Inv == Read = x+x
====
