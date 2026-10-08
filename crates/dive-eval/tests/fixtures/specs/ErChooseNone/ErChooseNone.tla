---- MODULE ErChooseNone ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME (CHOOSE x \in {1, 2} : x > 5) = 1
====
