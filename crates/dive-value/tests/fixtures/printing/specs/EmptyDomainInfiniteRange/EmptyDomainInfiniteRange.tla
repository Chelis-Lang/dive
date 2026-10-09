---- MODULE EmptyDomainInfiniteRange ----
EXTENDS Naturals, FiniteSets, TLC
ASSUME [{} -> Nat] = {<<>>} /\ Cardinality([{} -> Nat]) = 1 /\ IsFiniteSet([{} -> Nat])
VARIABLE x
Init == x=0
Next == x'=x
====
