---- MODULE Terminal ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x'=1
Inv == x=0
Live == <>(x=0)
Post == x/=x
Hook == FALSE
====
