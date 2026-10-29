---- MODULE RightErrorFirst ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
Cut == IF x = 0 THEN 1 ELSE Assert(FALSE,"substitute first")
I == INSTANCE MulError WITH v <- Cut
Inv == x = 0 \/ I!Right = 0
====
