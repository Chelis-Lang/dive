---- MODULE CmBitwise ----
EXTENDS Integers, Sequences, FiniteSets, TLC, Bitwise
ASSUME PrintT(5 ^^ 3)
ASSUME PrintT(3 ^^ 5)
ASSUME PrintT(0 ^^ 0)
ASSUME PrintT(2147483647 ^^ 1)
ASSUME PrintT(12 & 10)
ASSUME PrintT(12 | 10)
ASSUME PrintT((0 - 5) ^^ 3)
ASSUME PrintT(Not(5))
ASSUME PrintT(Not(0))
ASSUME PrintT(Not(1))
ASSUME PrintT(shiftR(16, 2))
ASSUME PrintT(shiftR(-16, 2))
ASSUME PrintT(shiftR(1, 33))
ASSUME PrintT(Not(-1))
ASSUME PrintT(shiftR(16, 33))
ASSUME PrintT(shiftR(16, 32))
====
