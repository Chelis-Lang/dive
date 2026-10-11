---- MODULE LazyView ----
EXTENDS Naturals,TLC
VARIABLE x
Init == x = {f \in [Nat -> {<<1>>,[a |-> 1,b |-> 1]}] : Print(0, TRUE)}
Next == UNCHANGED x
V == 0
====
