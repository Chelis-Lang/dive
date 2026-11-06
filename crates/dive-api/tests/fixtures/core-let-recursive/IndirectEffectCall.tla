---- MODULE IndirectEffectCall ----
EXTENDS Naturals, TLC
VARIABLE x
RECURSIVE Effect(_)
Effect(k) == IF k=0 THEN Print("effect",TRUE) ELSE Effect(k-1)
Init == x=0
Next == LET f[n \in 0..1] == IF n=0 THEN Effect(1) ELSE \E k \in {1,2}: f[n-1]
        IN f[1] /\ UNCHANGED x
Inv == x=0
====
