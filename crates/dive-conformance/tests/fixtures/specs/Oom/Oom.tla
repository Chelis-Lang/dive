---- MODULE Oom ----
EXTENDS Naturals, FiniteSets
VARIABLE x
Init == x = 0
Next == x' = x + 1 /\ x < 3
S == [i \in 1..3000000 |-> i]
Inv == Cardinality({S[i] : i \in 1..3000000}) > 0
====
