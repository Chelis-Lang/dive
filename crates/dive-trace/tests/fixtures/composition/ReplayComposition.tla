---- MODULE ReplayComposition ----
EXTENDS Integers
VARIABLE x
Init == x = 0
A == (x' = 1) \cdot (x' = 0)
Next == x' = x
OracleFacts == ENABLED A /\ ENABLED <<A>>_x /\ ~ENABLED <<x' = 0>>_x
Weak == WF_x(A)
Strong == SF_x(A)
====
