---- MODULE ErTupleIndexZero ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME <<1, 2>>[0] = 1
====
