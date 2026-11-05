---- MODULE CanonicalLazyDomains ----
EXTENDS Naturals, Sequences, FiniteSets
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Inv == /\ {} \in SUBSET (1..(100 + x))
       /\ [i \in 1..(30 + x) |-> 0] \in [1..(30 + x) -> {0, 1}]
       /\ <<>> \in Seq(1..1000000000)
       /\ Cardinality((1..1000000000) \X {0}) = 1000000000
====
