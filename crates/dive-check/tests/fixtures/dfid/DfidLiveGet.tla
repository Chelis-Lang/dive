---- MODULE DfidLiveGet ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x' = 0 /\ TLCGet("level") >= 0
====
