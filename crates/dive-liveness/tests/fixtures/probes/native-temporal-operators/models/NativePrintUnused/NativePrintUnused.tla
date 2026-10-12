---- MODULE NativePrintUnused ----
EXTENDS TLC
VARIABLE x
Init == x=TRUE
Next == UNCHANGED x
Use(F(_)) == []TRUE
Live == Use(PrintT)
====
