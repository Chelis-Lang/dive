---- MODULE Compound ----
EXTENDS Naturals,TLC
VARIABLES n,x,s
Init == n = 0 /\ x = 0 /\ s = {[a |-> 1,b |-> 1],<<1>>}
Next == n < 3 /\ n' = n+1 /\ x' = IF <<1>> \in s THEN n ELSE 0 /\ UNCHANGED s
Inv == n<3
Projection == n
====
