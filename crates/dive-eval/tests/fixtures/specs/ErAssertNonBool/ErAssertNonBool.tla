---- MODULE ErAssertNonBool ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME Assert(1, <<"x", 2>>)
====
