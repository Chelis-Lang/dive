---- MODULE NextViolation ----
EXTENDS Naturals,FiniteSets
VARIABLES x,s
Init == x = 0 /\ s = {0}
Next == x' \in 0..2 /\ s' = {x'}
Inv == x < 2 /\ Cardinality(s) = 1
====
