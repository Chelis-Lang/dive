---- MODULE UnknownRecursive ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == LET f[n \in 0..1] == IF n=0 THEN TRUE ELSE f[n]
        IN f[1] /\ UNCHANGED x
Inv == x=0
====
