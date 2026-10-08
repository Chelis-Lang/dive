---- MODULE ErExceptOutOfRange ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME [<<1, 2>> EXCEPT ![5] = 0] = <<1, 2>>
====
