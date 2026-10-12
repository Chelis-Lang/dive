---- MODULE Terminal ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x=0
Next == x'=IF x<512 THEN x+1 ELSE x
Inv == x=0
Live == <>(x=0)
Post == TLCGet("distinct") = 0
Hook == FALSE
====
