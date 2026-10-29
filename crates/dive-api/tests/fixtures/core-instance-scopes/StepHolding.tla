---- MODULE StepHolding ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
A == INSTANCE GuardMiddle WITH w <- x
B == INSTANCE GuardMiddle WITH w <- x + 2
Prop == [][ /\ A!Double' = 2*(x'+1) /\ B!Double' = 2*(x'+3)
             /\ A!Double = 2*(x+1) /\ B!Double = 2*(x+3) ]_x
====
