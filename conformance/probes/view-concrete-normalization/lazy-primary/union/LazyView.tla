---- MODULE LazyView ----
EXTENDS Naturals,TLC
VARIABLE x
Init == x = UNION {Nat}
Next == UNCHANGED x
V == 0
====
