---- MODULE EmptySymbolicQuantifier ----
EXTENDS Naturals, FiniteSets, TLC
ASSUME (\A f \in [Nat -> {}] : FALSE) /\ (\E f \in [[Nat -> {}] -> Nat] : f = << >>)
VARIABLE x
Init == x=0
Next == x'=x
====
