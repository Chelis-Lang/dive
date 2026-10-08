---- MODULE ErCmTlcGetUndefinedNamed ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME PrintT(TLCGet("s:nope"))
====
