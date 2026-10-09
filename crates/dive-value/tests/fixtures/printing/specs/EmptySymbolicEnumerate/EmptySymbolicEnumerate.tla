---- MODULE EmptySymbolicEnumerate ----
EXTENDS Naturals, FiniteSets, TLC
ASSUME [Nat -> {}] = {} /\ [[Nat -> {}] -> {1}] = {<<>>}
VARIABLE x
Init == x=0
Next == x'=x
====
