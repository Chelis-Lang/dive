---- MODULE ErExistsNonEnum ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME \E x \in Nat : x = 3
====
