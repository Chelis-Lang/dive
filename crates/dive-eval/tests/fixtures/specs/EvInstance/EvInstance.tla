---- MODULE EvInstance ----
EXTENDS Integers, Sequences, FiniteSets, TLC
I == INSTANCE Inner WITH K <- 5
J == INSTANCE Inner WITH K <- 7
L(n) == INSTANCE Inner WITH K <- n * 100
ASSUME PrintT(I!Double)
ASSUME PrintT(J!AddK(1))
ASSUME PrintT(I!AddK(J!Double))
ASSUME PrintT(I!Both)
ASSUME PrintT(J!Both)
ASSUME PrintT(L(3)!Double)
ASSUME PrintT(L(2)!Both)
====
