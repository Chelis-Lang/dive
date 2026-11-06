---- MODULE RegisterWitness ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x=0 /\ TLCSet(0,0)
Next == x=0 /\ LET f[n \in 0..1] == IF n=0 THEN TLCSet(0,TLCGet(0)+1) ELSE \E k \in {1,2}: f[n-1]
               IN f[1] /\ x'=TLCGet(0)
Inv == x<2
====
