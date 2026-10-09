---- MODULE SingletonInfiniteDomain ----
EXTENDS Naturals, FiniteSets, TLC
ASSUME Cardinality([Nat -> {1}]) = 1 /\ IsFiniteSet([Nat -> {1}])
VARIABLE x
Init == x=0
Next == x'=x
====
