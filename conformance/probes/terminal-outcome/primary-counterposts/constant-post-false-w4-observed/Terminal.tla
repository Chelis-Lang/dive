---- MODULE Terminal ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x=0
Next == x'=x
Inv == x=0
Live == <>(x=0)
Post == FALSE
Hook == FALSE
====
