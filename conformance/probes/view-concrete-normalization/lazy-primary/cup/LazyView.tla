---- MODULE LazyView ----
EXTENDS Naturals,TLC
VARIABLE x
Init == x = Nat \cup Nat
Next == UNCHANGED x
V == 0
====
