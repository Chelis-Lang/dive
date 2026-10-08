---- MODULE ErRecFcnDomain ----
EXTENDS Integers, Sequences, FiniteSets, TLC
f[n \in Nat] == IF n = 0 THEN 1 ELSE n * f[n - 1]
ASSUME f[-1] = 0
====
