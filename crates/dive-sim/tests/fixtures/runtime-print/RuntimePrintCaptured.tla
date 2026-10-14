---- MODULE RuntimePrintCaptured ----
EXTENDS Integers, TLC
VARIABLE x
Apply(Log(_), v) == Log(v)
Emit(v) == PrintT(v)
Init == x = 0
Next == Apply(Emit, x) /\ x' = x + 1
Safe == x < 2
====
