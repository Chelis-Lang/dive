---- MODULE LazyView ----
EXTENDS Naturals,TLC
VARIABLE x
Init == x = {f \in [Nat -> {2,1}] : Print(0, TRUE)}
Next == UNCHANGED x
V == 0
====
