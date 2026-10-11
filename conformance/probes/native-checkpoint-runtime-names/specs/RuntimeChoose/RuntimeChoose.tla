---- MODULE RuntimeChoose ----
EXTENDS Naturals,TLC
VARIABLE n,x,s
Init == n=0 /\ x \in 1..12 /\ s="unset"
Next == n<4 /\ n'=n+1 /\ x'=x /\ s'=(CHOOSE t \in {ToString(x+n),ToString(x+n+1)} : TRUE)
Inv == n<3
====
