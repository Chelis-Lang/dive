---- MODULE EvModelValues ----
EXTENDS Integers, Sequences, FiniteSets, TLC
CONSTANTS a, b, S
ASSUME PrintT(a = a)
ASSUME PrintT(a = b)
ASSUME PrintT(a \in S)
ASSUME PrintT(Cardinality(S))
ASSUME PrintT(S)
ASSUME PrintT(a = 1)
ASSUME PrintT([s \in S |-> 0][b])
====
