---- MODULE ErAssert ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME Assert(FALSE, "boom")
====
