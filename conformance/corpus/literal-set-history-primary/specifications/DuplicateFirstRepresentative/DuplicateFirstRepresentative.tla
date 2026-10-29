---- MODULE DuplicateFirstRepresentative ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == x = 0 \/ (LET S == {<<2>>, <<1>>, <<2>>} IN /\ Cardinality(S) = 2 /\ <<2>> \in S)
====
