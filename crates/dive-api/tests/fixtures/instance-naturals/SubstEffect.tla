---- MODULE SubstEffect ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
I == INSTANCE MulInner WITH v <- Print(x,x)
Inv == I!Left = 2*x
====
