---- MODULE QueryHolding ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
A == INSTANCE QueryMiddle WITH x <- x, limit <- 1
B == INSTANCE QueryMiddle WITH x <- x, limit <- 2
Inv == (A!Can = (x < 1)) /\ (B!Can = (x < 2))
====
