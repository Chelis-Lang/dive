---- MODULE TopRecState ----
EXTENDS Naturals
VARIABLE y
F[n \in 0..1] == IF n=0 THEN y ELSE F[n-1]+1
Read == F[1]
====
