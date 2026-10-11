---- MODULE LazyView ----
EXTENDS Naturals,TLC
VARIABLE x
Init == x = {i \in Nat : Print(i, TRUE)}
Next == UNCHANGED x
V == 0
====
