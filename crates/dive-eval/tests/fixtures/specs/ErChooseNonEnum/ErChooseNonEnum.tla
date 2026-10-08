---- MODULE ErChooseNonEnum ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME (CHOOSE x \in Nat : x > 3) = 4
====
