---- MODULE IncompleteView ----
EXTENDS Naturals,TLC
VARIABLES x,y,s
Init == x = 0 /\ s = {2,1}
Next == x' = 1 /\ s' = {2,1}
Projection == x
====
