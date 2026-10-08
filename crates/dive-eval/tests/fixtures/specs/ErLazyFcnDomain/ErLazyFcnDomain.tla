---- MODULE ErLazyFcnDomain ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME [n \in Nat |-> n][-1] = 0
====
