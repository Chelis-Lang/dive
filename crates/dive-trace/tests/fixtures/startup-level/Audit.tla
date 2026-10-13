---- MODULE Audit ----
EXTENDS Naturals, TLC
VARIABLE x
ASSUME TLCGet("level")=0
Init == x=0 /\ TLCGet("level")=0
Next == x'=x+1
Inv == TLCGet("level")<3
====
