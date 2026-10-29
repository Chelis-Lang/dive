---- MODULE NestedScopes ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
A == INSTANCE MulMiddle WITH w <- x
B == INSTANCE MulMiddle WITH w <- x + 2
Inv == A!Double = 2*(x+1) /\ B!Double = 2*(x+3)
====
