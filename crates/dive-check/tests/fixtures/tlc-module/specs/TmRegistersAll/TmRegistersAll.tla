---- MODULE TmRegistersAll ----
EXTENDS Naturals, TLC
VARIABLE x
ASSUME TLCSet(2, "a") /\ TLCSet("s:k", 3)
Init == x = 0
Next == x < 2 /\ PrintT(TLCGet("all")) /\ PrintT(TLCGet("all:named")) /\ TLCSet(x + 3, x) /\ x' = x + 1
====
