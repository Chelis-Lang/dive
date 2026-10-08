---- MODULE ErUnionEnum ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME Cardinality(UNION {Nat}) = 1
====
