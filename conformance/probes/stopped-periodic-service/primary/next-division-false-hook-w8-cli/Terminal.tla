---- MODULE Terminal ----
EXTENDS Naturals
VARIABLES x,y
Init == x = 0 /\ y = 0
Next == x' = 1 \div 0 /\ y' = y
Inv == x=0
Action == [][x'=x]_<<x,y>>
Live == <>(x=0)
Hook == FALSE
====
