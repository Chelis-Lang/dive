---- MODULE PrLazy ----
\* PrintT inside a function constructor, which TLC may build eagerly,
\* lazily or once per application: the core refuses it.
EXTENDS Naturals, TLC
VARIABLE f
Init == f = [i \in {1, 2} |-> PrintT(i)]
Next == UNCHANGED f
====
