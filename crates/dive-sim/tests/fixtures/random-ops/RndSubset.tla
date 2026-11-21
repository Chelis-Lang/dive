---- MODULE RndSubset ----
EXTENDS Integers, FiniteSets, Randomization
CONSTANT K
VARIABLE s
Init == s \in RandomSubset(3, SUBSET (1..4))
Next == s' = RandomSubset(K, 1..10)
Safe == Cardinality(s) <= 10
====
