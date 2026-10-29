---- MODULE SubstRight ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
I == INSTANCE MulInner WITH v <- x
Inv == I!Right = x*2
====
