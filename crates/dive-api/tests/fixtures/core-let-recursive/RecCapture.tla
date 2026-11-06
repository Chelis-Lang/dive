---- MODULE RecCapture ----
EXTENDS Naturals
CONSTANT K
Read(a) == LET f[n \in 0..2] == IF n=0 THEN K+a ELSE f[n-1]+K+a
           IN f[2]
====
