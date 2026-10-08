---- MODULE ErCmTlcSetBadKey ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME PrintT(TLCSet("nope", 1))
====
