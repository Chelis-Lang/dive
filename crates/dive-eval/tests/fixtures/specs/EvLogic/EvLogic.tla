---- MODULE EvLogic ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME PrintT(TRUE /\ FALSE)
ASSUME PrintT(FALSE \/ TRUE)
ASSUME PrintT(FALSE => (1 \div 0 = 1))
ASSUME PrintT(FALSE /\ (1 \div 0 = 1))
ASSUME PrintT(TRUE \/ (1 \div 0 = 1))
ASSUME PrintT(TRUE <=> FALSE)
ASSUME PrintT(FALSE <=> FALSE)
ASSUME PrintT(~TRUE)
ASSUME PrintT(TRUE => FALSE)
ASSUME PrintT(IF 1 > 2 THEN "a" ELSE "b")
ASSUME PrintT(IF TRUE THEN 1 ELSE 1 \div 0)
ASSUME PrintT(CASE 1 = 2 -> "a" [] 1 = 1 -> "b" [] OTHER -> "c")
ASSUME PrintT(CASE 1 = 2 -> "a" [] OTHER -> "c")
ASSUME PrintT(CASE 1 = 1 -> "first" [] 1 = 1 -> "second")
ASSUME PrintT(/\ TRUE
              /\ 1 = 1)
ASSUME PrintT(\/ FALSE
              \/ 1 = 1
              \/ 1 \div 0 = 1)
ASSUME PrintT(\E x \in {} : TRUE)
ASSUME PrintT(\A x \in {} : FALSE)
ASSUME PrintT(\E x \in 1..3 : x > 2)
ASSUME PrintT(\A x \in 1..3, y \in 1..2 : x + y > 1)
ASSUME PrintT(\E <<x, y>> \in {<<1, 2>>, <<3, 4>>} : x + y = 7)
ASSUME PrintT(\E x, y \in 1..3 : x = 2 /\ y = 3)
ASSUME PrintT(\A x \in {} , y \in 1..0 : FALSE)
ASSUME PrintT(\E x \in 1..3 : \E y \in 1..x : y = 3)
ASSUME PrintT(TRUE = TRUE)
ASSUME PrintT(BOOLEAN)
ASSUME PrintT(TRUE \in BOOLEAN)
====
