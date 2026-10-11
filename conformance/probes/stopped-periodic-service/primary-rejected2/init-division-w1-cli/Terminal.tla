---- MODULE Terminal ----
EXTENDS Naturals
VARIABLES x,y
Init == x = 1 \div 0 /\ y = 0
Next == x' = x /\ y' = y
Inv == x=0
Action == [][x'=x]_<<x,y>>
Live == <>(x=0)
====
