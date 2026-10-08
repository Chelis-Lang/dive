---- MODULE ErCmAtoiOverflow ----
EXTENDS Integers, Sequences, FiniteSets, TLC, IOUtils
ASSUME PrintT(atoi("2147483648"))
====
