---- MODULE ActionCount ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == LET f[n \in 0..1] == IF n=0 THEN TRUE ELSE \E k \in {1,2}: f[n-1]
        IN f[1] /\ UNCHANGED x
====
