---- MODULE GuardViolation ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
A == INSTANCE GuardMiddle WITH w <- x
B == INSTANCE GuardMiddle WITH w <- x + 2
Inv == A!Double = B!Double
====
