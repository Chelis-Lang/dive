---- MODULE Compound ----
EXTENDS Naturals,TLC
VARIABLES n,x,s
Init == n = 0 /\ x = 0 /\ s = [a |-> {[a |-> 1,b |-> 1],<<1>>}]
Next == n < 3 /\ n' = n+1 /\ x' = n /\ UNCHANGED s
Inv == n<3
Projection == n
====
