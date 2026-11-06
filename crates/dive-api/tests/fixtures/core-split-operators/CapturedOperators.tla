---- MODULE CapturedOperators ----
EXTENDS Naturals
VARIABLE x
Step(a, b) == b = IF a < 2 THEN a + 1 ELSE a
Unused(v) == v
I(k) == INSTANCE Inner WITH Start <- LAMBDA v: v = k
Init == I(0)!Init
Next == I(0)!Next
Inv == I(0)!Inv
====
