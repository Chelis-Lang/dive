---- MODULE ErCmTlcEvalError ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME PrintT(TLCEval(1 \div 0))
====
