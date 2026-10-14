---- MODULE RuntimePrintConfigured ----
EXTENDS Integers, TLC
VARIABLE x
CONSTANT Logger(_)
Emit(v) == PrintT(v)
Init == x = 0
Next == Logger(x) /\ x' = x + 1
Safe == x < 2
====
