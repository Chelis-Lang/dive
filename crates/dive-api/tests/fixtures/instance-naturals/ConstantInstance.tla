---- MODULE ConstantInstance ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
CONSTANT C
I == INSTANCE MulInner WITH v <- C
Double == I!Left
ASSUME Double = 6
Inv == Double = 6
====
