---- MODULE SplitOperators ----
EXTENDS Naturals, TLC
VARIABLE x
Start(v) == v = 0
Step(a, b) == b = IF a < 2 THEN a + 1 ELSE a
Unused(v) == Print("must stay unused", v)
INSTANCE Inner
====
