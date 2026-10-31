---- MODULE InitialTool ----
EXTENDS Integers, FiniteSets
VARIABLE x

Init == x=0
Next == x<2 /\ x'=x+1
Inv == IF 1 THEN TRUE ELSE FALSE
====
