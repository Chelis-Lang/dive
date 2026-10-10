---- MODULE DfidRedirectedGet ----
EXTENDS Naturals, TLC
CONSTANT Read(_)
VARIABLE x
Live(k) == TLCGet(k)
Init == x = 0
Next == x' = 0 /\ Read("level") >= 0
====
