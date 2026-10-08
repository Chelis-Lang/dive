---- MODULE ErRecFcnNonInt ----
EXTENDS Integers, Sequences, FiniteSets, TLC
f[n \in Nat] == n
ASSUME f["a"] = 0
====
