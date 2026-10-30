---- MODULE InitLevel ----
EXTENDS TLC
VARIABLES x,y
Init == (x=0 /\ y=0) \/ x=1
Next == UNCHANGED <<x,y>>
P == TLCGet("level")=0
====
