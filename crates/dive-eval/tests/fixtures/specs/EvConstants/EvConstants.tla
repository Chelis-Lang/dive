---- MODULE EvConstants ----
EXTENDS Integers, Sequences, FiniteSets, TLC
CONSTANTS N, Name, Flag, Op(_)
Repl(x) == x * 10
ASSUME PrintT(N + 1)
ASSUME PrintT(Name \o "d")
ASSUME PrintT(Flag)
ASSUME PrintT(Op(N))
ASSUME PrintT(1..N)
====
