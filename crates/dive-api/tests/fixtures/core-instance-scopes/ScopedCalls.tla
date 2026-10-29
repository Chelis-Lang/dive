---- MODULE ScopedCalls ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
A == INSTANCE GuardMiddle WITH w <- x
B == INSTANCE GuardMiddle WITH w <- x + 2
Inv == LET Check(n) == A!Double = 2*(x+1) /\ B!Double = 2*(x+3) /\ n = n
       IN \A i \in {1,2} : Check(i)
====
