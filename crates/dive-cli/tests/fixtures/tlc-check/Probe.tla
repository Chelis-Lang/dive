---- MODULE Probe ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x<2 /\ x'=x+1
Inv == x<2
Bad == x<0
====
