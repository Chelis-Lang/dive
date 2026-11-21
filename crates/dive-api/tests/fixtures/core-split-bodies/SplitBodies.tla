---- MODULE SplitBodies ----
EXTENDS Naturals
CONSTANT Digits
VARIABLE x
Init == x = 0
Step(p, q) ==
    LET delta == q - p
        advance(n) == n + delta
    IN /\ x = p
       /\ q >= p
       /\ x' = advance(x)
Next == \E i, j \in Digits : Step(i, j) \/ Step(j, i)
Bound == x < 3
====
