---- MODULE ErTailEmptyString ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME Tail("") = ""
====
