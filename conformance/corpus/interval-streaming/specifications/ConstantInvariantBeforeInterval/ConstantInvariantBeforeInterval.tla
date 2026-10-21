---- MODULE ConstantInvariantBeforeInterval ----
EXTENDS Naturals
VARIABLE x
Init == x \in 0..1000000000
Next == x'=x
Inv == FALSE
====
