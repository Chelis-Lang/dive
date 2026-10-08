---- MODULE ErExceptNonFcn ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME LET x == 1 IN [x EXCEPT ![1] = 2] = 1
====
