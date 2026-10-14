---- MODULE RuntimePrintDirect ----
EXTENDS Integers, TLC
VARIABLE x
Startup == PrintT("startup")
Init == x = 0 /\ PrintT("init")
Next == x' = Print(x, x + 1)
Safe == x < 2
====
