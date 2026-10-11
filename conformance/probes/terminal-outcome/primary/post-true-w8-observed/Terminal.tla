---- MODULE Terminal ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x'=x
Inv == x=0
Live == <>(x=0)
Post == x=x
Hook == FALSE
====
