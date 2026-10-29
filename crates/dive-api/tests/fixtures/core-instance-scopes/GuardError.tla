---- MODULE GuardError ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
A == INSTANCE GuardMiddle WITH w <- x
B == INSTANCE GuardMiddle WITH w <- IF x = 0 THEN x ELSE 1 \div 0
Inv == A!Double >= 0 /\ B!Double >= 0
====
