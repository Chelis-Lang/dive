---- MODULE LazyUnusedEffect ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
Ignore(unused, used) == used
I == INSTANCE MulInner WITH v <- Ignore(Print(99, 1 \div 0), x)
Inv == I!Left = 2*x
====
