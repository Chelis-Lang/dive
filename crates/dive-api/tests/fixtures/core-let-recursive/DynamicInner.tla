---- MODULE DynamicInner ----
EXTENDS Naturals
CONSTANT K
Read == LET f[n \in 0..1] == IF n=0 THEN K ELSE f[n-1]+K
        IN f[1]
====
