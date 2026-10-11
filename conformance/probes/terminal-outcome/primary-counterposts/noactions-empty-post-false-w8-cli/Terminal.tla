---- MODULE Terminal ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x \in {}
Next == x'=x
Inv == x=0
Live == <>(x=0)
Post == TLCGet("distinct") = 0
Hook == FALSE
====
