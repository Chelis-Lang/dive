---- MODULE ErChooseTupleMismatch ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME (CHOOSE <<x, y>> \in {1} : TRUE) = 1
====
