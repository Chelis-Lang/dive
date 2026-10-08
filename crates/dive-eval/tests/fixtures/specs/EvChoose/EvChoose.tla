---- MODULE EvChoose ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME PrintT(CHOOSE x \in 1..10 : x > 3)
ASSUME PrintT(CHOOSE x \in {3, 1, 2} : TRUE)
ASSUME PrintT(CHOOSE <<x, y>> \in {<<2, 1>>, <<1, 2>>} : TRUE)
ASSUME PrintT(CHOOSE s \in SUBSET {1, 2} : Cardinality(s) = 1)
ASSUME PrintT(CHOOSE f \in [{1, 2} -> {7, 8}] : TRUE)
ASSUME PrintT(CHOOSE x \in {{1, 2}, {3}, {}} : TRUE)
ASSUME PrintT(CHOOSE t \in {<<1, 2, 3>>, <<4>>} : TRUE)
ASSUME PrintT(CHOOSE x \in {{4}, {1, 2}} : TRUE)
ASSUME PrintT(CHOOSE r \in [a : {1, 2}, b : {3, 4}] : r.a + r.b = 5)
ASSUME PrintT(CHOOSE x \in {1, 2} \X {3, 4} : TRUE)
ASSUME PrintT(CHOOSE x \in -3..3 : x * x = 4)
ASSUME PrintT(CHOOSE x \in {TRUE, FALSE} : TRUE)
====
