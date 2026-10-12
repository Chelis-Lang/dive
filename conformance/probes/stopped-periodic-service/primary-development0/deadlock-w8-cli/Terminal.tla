---- MODULE Terminal ----
EXTENDS Naturals
VARIABLES x,y
Init == x = 0 /\ y = 0
Next == FALSE
Inv == x=0
Action == x'=x
Live == <>(x=0)
====
