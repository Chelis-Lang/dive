---- MODULE ErCmQuantifyNonSet ----
EXTENDS Integers, Sequences, FiniteSets, TLC, FiniteSetsExt
ASSUME PrintT(Quantify(3, LAMBDA x: TRUE))
====
