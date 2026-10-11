---- MODULE Terminal ----
EXTENDS Naturals
VARIABLES x,y
Init == x = 0 /\ y = 0
Next == x' = 1
Inv == x=0
Action == x'=x
Live == <>(x=0)
====
