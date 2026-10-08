---- MODULE ErCmTlcGetArgError ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME PrintT(TLCGet(1 \div 0))
====
