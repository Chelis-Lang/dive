---- MODULE RndSubsetNegative ----
EXTENDS Integers, FiniteSets, Randomization
VARIABLE s
Init == s = {}
Next == s' = RandomSubset(-1, 1..10)
Safe == Cardinality(s) <= 10
====
