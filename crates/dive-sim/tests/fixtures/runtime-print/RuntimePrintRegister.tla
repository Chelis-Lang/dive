---- MODULE RuntimePrintRegister ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Next == PrintT(x) /\ TLCSet(0, 1) /\ x' = x + 1
Safe == x < 2
====
