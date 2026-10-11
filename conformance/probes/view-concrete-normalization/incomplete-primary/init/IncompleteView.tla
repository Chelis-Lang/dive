---- MODULE IncompleteView ----
EXTENDS Naturals,TLC
VARIABLES x,y,s
Init == x = 0 /\ s = {<<1>>,[a |-> 1,b |-> 1]}
Next == x' = 1 /\ s' = {<<1>>,[a |-> 1,b |-> 1]}
Projection == x
====
