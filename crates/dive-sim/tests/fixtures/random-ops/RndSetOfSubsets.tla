---- MODULE RndSetOfSubsets ----
EXTENDS Integers, FiniteSets, Randomization
VARIABLE s
Init == s = {}
Next == s' = RandomSetOfSubsets(3, 2, 1..5)
Safe == Cardinality(s) <= 3
====
