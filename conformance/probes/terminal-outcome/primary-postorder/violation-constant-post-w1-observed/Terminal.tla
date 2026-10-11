---- MODULE Terminal ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x'=1
Live == <>(x=2)
Post == FALSE
====
