---- MODULE ErFilterEnumNat ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME Cardinality({x \in Nat : x < 3}) = 3
====
