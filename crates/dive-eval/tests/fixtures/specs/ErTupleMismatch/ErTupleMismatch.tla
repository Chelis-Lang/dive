---- MODULE ErTupleMismatch ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME \E <<x, y>> \in {<<1>>} : TRUE
====
