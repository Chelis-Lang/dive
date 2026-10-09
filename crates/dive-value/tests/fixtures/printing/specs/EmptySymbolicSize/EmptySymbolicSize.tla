---- MODULE EmptySymbolicSize ----
EXTENDS Naturals, FiniteSets, TLC
ASSUME Cardinality([Nat -> {}]) = 0 /\ IsFiniteSet([Nat -> {}]) /\ Cardinality([[Nat -> {}] -> {1}]) = 1 /\ IsFiniteSet([[Nat -> {}] -> {1}])
VARIABLE x
Init == x=0
Next == x'=x
====
