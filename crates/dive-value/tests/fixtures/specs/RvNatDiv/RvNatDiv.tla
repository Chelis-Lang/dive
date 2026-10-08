---- MODULE RvNatDiv ----
EXTENDS Naturals, TLC
ASSUME PrintT("#a") /\ PrintT((0 - 1) \div 2)
ASSUME PrintT("#b") /\ PrintT((0 - 7) \div 2)
ASSUME PrintT("#c") /\ PrintT(7 \div (0 - 2))
ASSUME PrintT("#d") /\ PrintT(1 \div (0 - 2))
ASSUME PrintT("#e") /\ PrintT((0 - 1) % 2)
ASSUME PrintT("#f") /\ PrintT((0 - 2147483647 - 1) \div (0 - 1))
====
