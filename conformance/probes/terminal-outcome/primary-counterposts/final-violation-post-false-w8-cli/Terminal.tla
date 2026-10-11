---- MODULE Terminal ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x=0
Next == x'=1
Inv == x=0
Live == <>(x=2)
Post == TLCGet("distinct") = 0
Hook == FALSE
====
