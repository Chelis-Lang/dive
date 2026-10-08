---- MODULE RvIntDiv ----
EXTENDS Integers, TLC
ASSUME PrintT("#a") /\ PrintT((-1) \div 2)
ASSUME PrintT("#b") /\ PrintT((-7) \div 2)
ASSUME PrintT("#c") /\ PrintT(7 \div (-2))
ASSUME PrintT("#d") /\ PrintT(1 \div (-2))
ASSUME PrintT("#e") /\ PrintT((-1) % 2)
====
