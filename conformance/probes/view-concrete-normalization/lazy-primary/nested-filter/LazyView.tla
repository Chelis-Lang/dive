---- MODULE LazyView ----
EXTENDS Naturals,TLC
VARIABLE x
Init == x = <<{i \in Nat : Print(i, TRUE)}, 1>>
Next == UNCHANGED x
V == 0
====
