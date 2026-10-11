---- MODULE LazyView ----
EXTENDS Naturals,TLC
VARIABLE x
Init == x = Nat \cap Nat
Next == UNCHANGED x
V == 0
====
