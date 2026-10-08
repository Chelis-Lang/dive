---- MODULE ErChooseBodyNonBool ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME (CHOOSE x \in {1} : x) = 1
====
