---- MODULE ErChooseUnbounded ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME (CHOOSE x : TRUE) = 1
====
