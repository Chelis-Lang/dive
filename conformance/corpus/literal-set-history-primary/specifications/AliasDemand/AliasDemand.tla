---- MODULE AliasDemand ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == x = 0 \/ (LET S == {<<1, TRUE>>, <<2, 3>>, <<2, 3>>} IN /\ Cardinality(S) = 2 /\ <<2, 3>> \in S)
====
